# Sourced by run.sh and test-watchdog.sh. The caller defines `book` (run.sh:
# the turn ledger) and CAP.
#
# guard_step NAME RESERVED TAP KILL_PATTERN LIMIT_SECS CMD...
# Runs CMD in the background and polls the tap's agent_start count every
# WATCH_POLL seconds (default 2). When the count passes RESERVED, or the step
# outlives LIMIT_SECS, it kills: every process whose command line matches
# KILL_PATTERN (Zed's env loader setsid()s a re-executed test binary, so only
# its path finds every copy), the OMP pids named by the tap files
# (omp.<pid>.jsonl) and run.sh's not yet exec'd wrappers ($TAP.omp), then
# CMD's process tree, then the OMPs again; books the real count and exits 1.
# Otherwise it returns CMD's status.

kill_tree() {
  local child
  for child in $(pgrep -P "$1" 2>/dev/null); do kill_tree "$child"; done
  kill -9 "$1" 2>/dev/null || true
}

# The OMP pids named by the tap files, and run.sh's wrappers ($tap.omp) that
# have not exec'd OMP yet, so have no tap file.
kill_omps() {
  local f
  pkill -9 -f "$1.omp" 2>/dev/null || true
  for f in "$1"/omp.*.jsonl; do
    [ -e "$f" ] || continue
    f=${f##*/omp.}
    kill -9 "${f%.jsonl}" 2>/dev/null || true
  done
}

guard_step() {
  local name=$1 reserved=$2 tap=$3 pattern=$4 limit=$5 pid n why start=$SECONDS status=0
  shift 5
  "$@" &
  pid=$!
  while kill -0 "$pid" 2>/dev/null; do
    sleep "${WATCH_POLL:-2}"
    n=$($CAP count "$tap")
    if [ "$n" -gt "$reserved" ]; then why="$n turns tapped, $reserved reserved"
    elif [ $((SECONDS - start)) -ge "$limit" ]; then why="over its ${limit}s wall clock"
    else continue
    fi
    pkill -9 -f "$pattern" 2>/dev/null || true
    kill_omps "$tap"
    kill_tree "$pid"
    wait "$pid" 2>/dev/null || true
    # An OMP the dead binary started just before the kill may have opened its
    # tap only now.
    sleep 0.2
    kill_omps "$tap"
    n=$($CAP count "$tap")
    echo "$name: killed, $why; booking $n" >&2
    # run.sh's book exits 0 when the cap is reached; a kill is still a failure.
    (book "$name" "$reserved" "$n") || true
    exit 1
  done
  wait "$pid" || status=$?
  return "$status"
}
