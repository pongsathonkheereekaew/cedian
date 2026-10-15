#!/usr/bin/env bash
# guard_step against a fake runaway: no cargo, no OMP, no model. The fake test
# binary detaches into its own session (as Zed's env loader does) and starts a
# fake OMP that keeps tapping agent_start frames. Run: script/cedian-bench/test-watchdog.sh
set -euo pipefail
BENCH=$(cd "$(dirname "$0")" && pwd)
CAP="python3 $BENCH/cap.py"
. "$BENCH/watchdog.sh"

tmp=$(mktemp -d)
trap 'pkill -9 -f "$tmp/" 2>/dev/null || true; rm -rf "$tmp"' EXIT
mkdir -p "$tmp/deps" "$tmp/tap"
# BOOK_STATUS mimics run.sh's book, which exits 0 once the cap is reached.
book() { echo "$*" > "$tmp/booked"; [ -z "${BOOK_STATUS:-}" ] || exit "$BOOK_STATUS"; }
fail() { echo "FAIL: $*" >&2; exit 1; }

cat > "$tmp/fake-omp" <<'EOF'
#!/bin/bash
echo $$ > "$2"
exec > "$1/omp.$$.jsonl"
while :; do echo '{"type":"agent_start"}'; sleep 0.2; done
EOF
cat > "$tmp/deps/lane-0123abcd" <<EOF
#!/bin/bash
while :; do $tmp/fake-omp $tmp/tap $tmp/omp.pid & wait; done
EOF
# A late tap: run.sh's wrapper ($tap.omp) has not exec'd OMP or opened its
# tap file yet when the watchdog fires.
cat > "$tmp/tap.omp" <<EOF
#!/bin/bash
sleep 2
exec $tmp/fake-omp $tmp/tap $tmp/omp.pid
EOF
cat > "$tmp/deps/late-0123abcd" <<EOF
#!/bin/bash
while :; do $tmp/tap.omp & sleep 0.3; done
EOF
chmod 755 "$tmp/fake-omp" "$tmp/deps/lane-0123abcd" "$tmp/tap.omp" "$tmp/deps/late-0123abcd"

# The step: start the binary in a new session, detached, then sit like cargo.
step() {
  (perl -MPOSIX -e 'POSIX::setsid(); exec @ARGV' "$tmp/deps/${1:-lane}-0123abcd" &)
  sleep 600
}

check() {
  local status=$1 booked real omp
  [ "$status" = 1 ] || fail "exit $status, want 1"
  sleep 2.5
  ! pgrep -f "$tmp/deps/" >/dev/null || fail "the fake test binary still runs"
  ! pgrep -f "$tmp/tap.omp" >/dev/null || fail "an OMP wrapper still runs"
  ! pgrep -f "$tmp/fake-omp" >/dev/null || fail "the fake OMP still runs"
  omp=$(cat "$tmp/omp.pid")
  ! kill -0 "$omp" 2>/dev/null || fail "OMP pid $omp still runs"
  real=$($CAP count "$tmp/tap")
  booked=$(cat "$tmp/booked")
  [ "$booked" = "$2 $3 $real" ] || fail "booked '$booked', want '$2 $3 $real'"
  echo "ok: $2 killed, exit 1, booked $real"
}

status=0
(WATCH_POLL=0.5 guard_step record-lane 2 "$tmp/tap" "$tmp/deps/lane-" 60 step) || status=$?
check "$status" record-lane 2

# The wall clock: a reservation the fake never passes in time.
rm -rf "$tmp/tap" "$tmp/booked"; mkdir -p "$tmp/tap"
status=0
(WATCH_POLL=0.5 guard_step record-slow 100000 "$tmp/tap" "$tmp/deps/lane-" 2 step) || status=$?
check "$status" record-slow 100000

# An OMP whose tap file did not exist yet at the kill.
rm -rf "$tmp/tap" "$tmp/booked" "$tmp/omp.pid"; mkdir -p "$tmp/tap"
status=0
(WATCH_POLL=0.5 guard_step record-late 2 "$tmp/tap" "$tmp/deps/late-" 60 step late) || status=$?
check "$status" record-late 2

# A kill that reaches the cap still exits 1.
rm -rf "$tmp/tap" "$tmp/booked"; mkdir -p "$tmp/tap"
status=0
(BOOK_STATUS=0 WATCH_POLL=0.5 guard_step record-cap 2 "$tmp/tap" "$tmp/deps/lane-" 60 step) || status=$?
check "$status" record-cap 2

# A step that ends inside its reservation keeps its own status.
status=0
(WATCH_POLL=0.5 guard_step record-ok 2 "$tmp/empty" "$tmp/deps/lane-" 60 sh -c 'exit 7') || status=$?
[ "$status" = 7 ] || fail "a finished step returned $status, want 7"
echo "ok: a finished step keeps its status"
