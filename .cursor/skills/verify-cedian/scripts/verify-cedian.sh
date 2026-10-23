#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../../.." && pwd)"
DATA_DIR="${CEDIAN_DATA_DIR:-/tmp/verify-cedian/data}"
EVIDENCE_DIR="${CEDIAN_EVIDENCE_DIR:-/tmp/verify-cedian/evidence}"
CEDIA_DIR="${CEDIA_CHECKOUT:-/home/ubuntu/cedia}"
PID_FILE="${CEDIAN_PID_FILE:-/tmp/verify-cedian/cedian.pid}"
DAEMON_PID_FILE="${CEDIAN_DAEMON_PID_FILE:-/tmp/verify-cedian/daemon.pid}"
LIVE_ENV="${CEDIAN_LIVE_ENV:-/tmp/verify-cedian/live.env}"
LIVE_DIR="${CEDIAN_LIVE_DIR:-/tmp/verify-cedian/live}"
REPLAY="${CEDIAN_REPLAY:-$ROOT/crates/cedian_protocol/cedia/sessions/edit-and-check/stream.jsonl}"
WINDOW_ID_FILE="${EVIDENCE_DIR}/window.id"
SHELL_PID_FILE="${EVIDENCE_DIR}/shell.pids"
LOG_FILE="${EVIDENCE_DIR}/cedian.log"
ARGV_FILE="${EVIDENCE_DIR}/cedia.argv"
PIN_FILE="$ROOT/crates/cedian_protocol/cedia/REV"
FAKE_BIN="$ROOT/crates/cedian_daemon/tests/fake/cedia"

FEATURES="agent-pane|editor-pane|spawn-cedia|terminal-pane|review-pane|desktop-settings|composer|sidebar|project-tab"

load_live() {
  if [[ -f "$LIVE_ENV" ]]; then
    set -a
    source "$LIVE_ENV"
    set +a
  fi
}

refuse_user_state() {
  local target="$1"
  local resolved blocked base
  resolved="$(readlink -m "$target")"
  for blocked in "${HOME}/.cedia" /home/ubuntu/.cedia /root/.cedia; do
    base="$(readlink -m "$blocked")"
    if [[ "$resolved" == "$base" || "$resolved" == "$base"/* ]]; then
      echo "refusing path inside ${blocked}: ${target}" >&2
      exit 2
    fi
  done
}

home_cedia_state() {
  if [[ -e "${HOME}/.cedia" || -e /home/ubuntu/.cedia ]]; then
    echo present
  else
    echo absent
  fi
}

cedian_bin_path() {
  if [[ -n "${CEDIAN_BIN:-}" && -x "${CEDIAN_BIN}" ]]; then
    echo "$CEDIAN_BIN"
    return
  fi
  local candidate
  for candidate in "$ROOT/target/debug/cedian" "$ROOT/target/release/cedian"; do
    if [[ -x "$candidate" ]]; then
      echo "$candidate"
      return
    fi
  done
  if command -v cedian >/dev/null 2>&1; then
    command -v cedian
  fi
}

cedia_pin() {
  if [[ -f "$PIN_FILE" ]]; then
    tr -d '[:space:]' <"$PIN_FILE"
  else
    echo missing
  fi
}

cedia_checkout_rev() {
  if git -C "$CEDIA_DIR" rev-parse HEAD >/dev/null 2>&1; then
    git -C "$CEDIA_DIR" rev-parse HEAD
  else
    echo missing
  fi
}

project_slug() {
  python3 -c 'import sys
t = sys.argv[1][1:] if sys.argv[1].startswith("/") else sys.argv[1]
out = []
run = False
for c in t:
    if c.isascii() and (c.isalnum() or c in "._-"):
        out.append(c)
        run = False
    elif not run:
        out.append("-")
        run = True
print("".join(out) or "root")' "$1"
}

data_dir_entries() {
  if [[ -d "$DATA_DIR" ]]; then
    find "$DATA_DIR" -mindepth 1 -maxdepth 1 | wc -l | tr -d ' '
  else
    echo missing
  fi
}

report_common() {
  local bin
  bin="$(cedian_bin_path || true)"
  echo "cedian_bin=${bin}"
  echo "cedia_pin=$(cedia_pin)"
  echo "cedia_checkout=${CEDIA_DIR}"
  echo "cedia_checkout_rev=$(cedia_checkout_rev)"
  echo "cedia_bin=${CEDIAN_CEDIA_BIN:-unset}"
  echo "cedia_kind=${CEDIAN_CEDIA_KIND:-unset}"
  echo "cedia_home=${CEDIA_HOME:-unset}"
  echo "state_dir=${CEDIAN_STATE_DIR:-unset}"
  echo "project=${CEDIAN_PROJECT:-unset}"
  echo "data_dir=${DATA_DIR}"
  echo "data_dir_entries=$(data_dir_entries)"
  echo "home_cedia=$(home_cedia_state)"
}

window_pid() {
  local pid
  if [[ ! -f "$PID_FILE" ]]; then
    return 1
  fi
  pid="$(tr -d '[:space:]' <"$PID_FILE")"
  if [[ "$pid" =~ ^[0-9]+$ ]] && kill -0 "$pid" 2>/dev/null; then
    echo "$pid"
    return 0
  fi
  return 1
}

print_ready() {
  local bin pid
  bin="$(cedian_bin_path || true)"
  if [[ -z "$bin" ]]; then
    echo "ready=no"
    echo "reason=no cedian binary; cargo build -p cedian_window from the repo root"
    return 1
  fi
  if [[ "$(home_cedia_state)" != absent ]]; then
    echo "ready=no"
    echo "reason=home cedia is present"
    return 1
  fi
  if ! pid="$(window_pid)"; then
    echo "ready=no"
    echo "reason=window is not running"
    return 1
  fi
  if [[ -z "${CEDIAN_STATE_DIR:-}" || ! -S "${CEDIAN_STATE_DIR}/daemon.sock" ]]; then
    echo "ready=no"
    echo "reason=daemon socket is missing"
    return 1
  fi
  echo "ready=yes"
  echo "pid=${pid}"
  echo "daemon_sock=${CEDIAN_STATE_DIR}/daemon.sock"
  return 0
}

prepare_display() {
  if [[ -z "${XDG_RUNTIME_DIR:-}" || ! -d "${XDG_RUNTIME_DIR}" ]]; then
    export XDG_RUNTIME_DIR="/tmp/verify-cedian/runtime"
    mkdir -p "$XDG_RUNTIME_DIR"
    chmod 700 "$XDG_RUNTIME_DIR"
  fi
  if [[ -z "${VK_DRIVER_FILES:-}${VK_ICD_FILENAMES:-}" && -f /usr/share/vulkan/icd.d/lvp_icd.json ]]; then
    export VK_DRIVER_FILES="/usr/share/vulkan/icd.d/lvp_icd.json"
    echo "vulkan_icd=lavapipe"
  elif [[ -n "${VK_DRIVER_FILES:-}" ]]; then
    echo "vulkan_icd=${VK_DRIVER_FILES}"
  elif [[ -n "${VK_ICD_FILENAMES:-}" ]]; then
    echo "vulkan_icd=${VK_ICD_FILENAMES}"
  else
    echo "vulkan_icd=unset"
  fi
}

content_window_ids() {
  if ! command -v xwininfo >/dev/null 2>&1; then
    return 0
  fi
  xwininfo -root -tree 2>/dev/null | awk '/1280x800\+/ { print $1 }' | sort -u
}

window_name() {
  if ! command -v xwininfo >/dev/null 2>&1; then
    echo none
    return 0
  fi
  xwininfo -root -tree 2>/dev/null | awk -F'"' '/1280x800\+/ { print $2; exit }'
}

remember_window() {
  local before="$1"
  local id
  id="$(comm -13 "$before" <(content_window_ids) | head -n 1 || true)"
  if [[ -z "$id" ]]; then
    id="$(content_window_ids | head -n 1 || true)"
  fi
  if [[ -z "$id" ]]; then
    echo "window_id=none"
    rm -f "$WINDOW_ID_FILE"
    return 0
  fi
  echo "$id" >"$WINDOW_ID_FILE"
  echo "window_id=${id}"
  echo "window_name=$(window_name)"
}

stop_pid() {
  local pid="$1"
  if [[ "$pid" =~ ^[0-9]+$ ]] && kill -0 "$pid" 2>/dev/null; then
    kill "$pid" 2>/dev/null || true
    local i
    for ((i = 0; i < 20; i++)); do
      if ! kill -0 "$pid" 2>/dev/null; then
        return 0
      fi
      sleep 0.1
    done
    kill -9 "$pid" 2>/dev/null || true
  fi
}

descendants() {
  local pid="$1"
  local child
  for child in $(ps -o pid= --ppid "$pid" 2>/dev/null || true); do
    echo "$child"
    descendants "$child"
  done
}

choose_cedia() {
  local bun_bin="${HOME}/.bun/bin/bun"
  local wrapper="/tmp/verify-cedian/cedia-pinned.sh"
  CEDIAN_CEDIA_KIND=fake
  CEDIAN_CEDIA_BIN="$FAKE_BIN"
  if [[ -n "${CEDIAN_CEDIA_BIN_OVERRIDE:-}" && -x "${CEDIAN_CEDIA_BIN_OVERRIDE}" ]]; then
    CEDIAN_CEDIA_BIN="$CEDIAN_CEDIA_BIN_OVERRIDE"
    CEDIAN_CEDIA_KIND=override
    return 0
  fi
  if [[ -x "$CEDIA_DIR/bin/cedia" ]]; then
    CEDIAN_CEDIA_BIN="$CEDIA_DIR/bin/cedia"
    CEDIAN_CEDIA_KIND=pinned
    return 0
  fi
  if [[ -x "$bun_bin" && -f "$CEDIA_DIR/src/cli.ts" ]]; then
    if "$bun_bin" "$CEDIA_DIR/src/cli.ts" --version --json >"$EVIDENCE_DIR/cedia-version.json" 2>"$EVIDENCE_DIR/cedia-version.err"; then
      cat >"$wrapper" <<EOF
#!/usr/bin/env bash
export PATH="\$HOME/.bun/bin:\$PATH"
exec "\$HOME/.bun/bin/bun" "$CEDIA_DIR/src/cli.ts" "\$@"
EOF
      chmod +x "$wrapper"
      CEDIAN_CEDIA_BIN="$wrapper"
      CEDIAN_CEDIA_KIND=pinned
      return 0
    fi
  fi
  return 0
}

write_live_env() {
  mkdir -p "$(dirname "$LIVE_ENV")"
  cat >"$LIVE_ENV" <<EOF
CEDIA_HOME=$(printf '%q' "$CEDIA_HOME")
CEDIAN_STATE_DIR=$(printf '%q' "$CEDIAN_STATE_DIR")
CEDIAN_PROJECT=$(printf '%q' "$CEDIAN_PROJECT")
CEDIAN_NO_LOGIN_ENV=1
CEDIAN_CEDIA_BIN=$(printf '%q' "$CEDIAN_CEDIA_BIN")
CEDIAN_DAEMON_BIN=$(printf '%q' "$CEDIAN_DAEMON_BIN")
CEDIAN_CEDIA_KIND=$(printf '%q' "$CEDIAN_CEDIA_KIND")
CEDIAN_PROJECT_SLUG=$(printf '%q' "$CEDIAN_PROJECT_SLUG")
FAKE_FIXTURE=$(printf '%q' "${FAKE_FIXTURE:-}")
FAKE_THREADS=$(printf '%q' "${FAKE_THREADS:-}")
FAKE_SESSIONS=$(printf '%q' "${FAKE_SESSIONS:-}")
CEDIA_FAUX_SCRIPT=$(printf '%q' "${CEDIA_FAUX_SCRIPT:-}")
CEDIA_SANDBOX=$(printf '%q' "${CEDIA_SANDBOX:-}")
EOF
}

cmd_launch() {
  local bin before pid i id
  refuse_user_state "$DATA_DIR"
  refuse_user_state "$EVIDENCE_DIR"
  refuse_user_state "$LIVE_DIR"
  if pid="$(window_pid || true)" && [[ -n "${pid}" ]]; then
    stop_pid "$pid"
  fi
  if [[ -f "$DAEMON_PID_FILE" ]]; then
    stop_pid "$(tr -d '[:space:]' <"$DAEMON_PID_FILE")" || true
  fi
  rm -f "$PID_FILE" "$DAEMON_PID_FILE" "$LIVE_ENV"
  rm -rf "$DATA_DIR" "$LIVE_DIR"
  mkdir -p "$DATA_DIR" "$EVIDENCE_DIR" "$LIVE_DIR/home" "$LIVE_DIR/state" "$LIVE_DIR/project"
  echo "command=launch"
  CEDIA_HOME="$LIVE_DIR/home"
  CEDIAN_STATE_DIR="$LIVE_DIR/state"
  CEDIAN_PROJECT="$LIVE_DIR/project"
  CEDIAN_PROJECT_SLUG="$(project_slug "$CEDIAN_PROJECT")"
  CEDIAN_DAEMON_BIN="$ROOT/target/debug/cedian-daemon"
  choose_cedia
  printf '%s\n' '{"sandbox":{"mode":"off"}}' >"$CEDIA_HOME/settings.json"
  if [[ "$CEDIAN_CEDIA_KIND" == fake ]]; then
    FAKE_FIXTURE="$ROOT/crates/cedian_protocol/cedia/sessions/edit-and-check"
    FAKE_THREADS="$CEDIA_HOME/projects/$CEDIAN_PROJECT_SLUG/threads"
    FAKE_SESSIONS="$CEDIA_HOME/sessions/$CEDIAN_PROJECT_SLUG"
    mkdir -p "$FAKE_THREADS" "$FAKE_SESSIONS"
    unset CEDIA_FAUX_SCRIPT || true
    CEDIA_SANDBOX=""
  else
    FAKE_FIXTURE=""
    FAKE_THREADS=""
    FAKE_SESSIONS=""
    CEDIA_FAUX_SCRIPT="$LIVE_DIR/faux.json"
    CEDIA_SANDBOX=off
    printf '%s\n' '[{"blocks":[{"type":"text","text":"say good"}]}]' >"$CEDIA_FAUX_SCRIPT"
  fi
  export CEDIA_HOME CEDIAN_STATE_DIR CEDIAN_PROJECT CEDIAN_NO_LOGIN_ENV=1
  export CEDIAN_CEDIA_BIN CEDIAN_DAEMON_BIN CEDIAN_CEDIA_KIND CEDIAN_PROJECT_SLUG
  export FAKE_FIXTURE FAKE_THREADS FAKE_SESSIONS CEDIA_FAUX_SCRIPT CEDIA_SANDBOX
  write_live_env
  report_common
  bin="$(cedian_bin_path || true)"
  if [[ -z "$bin" ]]; then
    echo "ready=no"
    echo "reason=no cedian binary; cargo build -p cedian_window from the repo root"
    echo "pid_file=none"
    exit 1
  fi
  if [[ "$(home_cedia_state)" != absent ]]; then
    echo "ready=no"
    echo "reason=home cedia is present"
    echo "pid_file=none"
    exit 1
  fi
  if [[ -z "${DISPLAY:-}" ]]; then
    echo "ready=no"
    echo "reason=DISPLAY is unset"
    echo "pid_file=none"
    exit 1
  fi
  if [[ ! -x "$CEDIAN_DAEMON_BIN" ]]; then
    echo "ready=no"
    echo "reason=no cedian-daemon binary; cargo build -p cedian_daemon from the repo root"
    echo "pid_file=none"
    exit 1
  fi
  prepare_display
  echo "display=${DISPLAY}"
  before="$(mktemp)"
  content_window_ids >"$before"
  : >"$LOG_FILE"
  nohup "$bin" --cwd "$CEDIAN_PROJECT" >"$LOG_FILE" 2>&1 </dev/null &
  pid="$!"
  echo "$pid" >"$PID_FILE"
  for ((i = 0; i < 40; i++)); do
    if ! kill -0 "$pid" 2>/dev/null; then
      echo "ready=no"
      echo "reason=cedian exited during launch"
      echo "pid=${pid}"
      echo "log=${LOG_FILE}"
      rm -f "$before"
      exit 1
    fi
    if [[ -S "${CEDIAN_STATE_DIR}/daemon.sock" ]]; then
      id="$(comm -13 "$before" <(content_window_ids) | head -n 1 || true)"
      if [[ -n "$id" ]]; then
        break
      fi
    fi
    sleep 0.25
  done
  if [[ "$(home_cedia_state)" != absent ]]; then
    stop_pid "$pid"
    rm -f "$PID_FILE"
    rm -rf "${HOME}/.cedia" /home/ubuntu/.cedia
    echo "ready=no"
    echo "reason=cedian created ~/.cedia; removed it"
    echo "home_cedia=$(home_cedia_state)"
    rm -f "$before"
    exit 1
  fi
  remember_window "$before"
  rm -f "$before"
  local daemon_pid
  daemon_pid="$(ps -o pid=,comm= --ppid "$pid" | awk '$2=="cedian-daemon" { print $1; exit }')"
  if [[ -n "$daemon_pid" ]]; then
    echo "$daemon_pid" >"$DAEMON_PID_FILE"
    echo "daemon_pid=${daemon_pid}"
  else
    echo "daemon_pid=none"
  fi
  if [[ ! -S "${CEDIAN_STATE_DIR}/daemon.sock" || ! -s "$WINDOW_ID_FILE" ]]; then
    echo "ready=no"
    echo "reason=window or daemon did not come up"
    echo "pid=${pid}"
    echo "log=${LOG_FILE}"
    exit 1
  fi
  echo "ready=yes"
  echo "pid=${pid}"
  echo "pid_file=${PID_FILE}"
  echo "log=${LOG_FILE}"
  echo "daemon_sock=${CEDIAN_STATE_DIR}/daemon.sock"
  if "$CEDIAN_CEDIA_BIN" --version --json >"$EVIDENCE_DIR/cedia-version.json" 2>"$EVIDENCE_DIR/cedia-version.err"; then
    echo "cedia_version=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["version"])' "$EVIDENCE_DIR/cedia-version.json")"
    echo "cedia_protocol=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["protocol"])' "$EVIDENCE_DIR/cedia-version.json")"
  else
    echo "cedia_version=unavailable"
  fi
}

cmd_doctor() {
  refuse_user_state "$DATA_DIR"
  echo "command=doctor"
  report_common
  if print_ready; then
    exit 0
  fi
  exit 1
}

capture_window() {
  local dest="$1"
  local id=""
  mkdir -p "$(dirname "$dest")"
  if [[ -f "$WINDOW_ID_FILE" ]]; then
    id="$(tr -d '[:space:]' <"$WINDOW_ID_FILE")"
  fi
  if [[ -n "$id" ]] && command -v import >/dev/null 2>&1; then
    xdotool windowactivate --sync "$id" >/dev/null 2>&1 || true
    if import -window "$id" "$dest" 2>/dev/null; then
      echo "screenshot=${dest}"
      echo "screenshot_scope=window"
      return 0
    fi
  fi
  if command -v import >/dev/null 2>&1; then
    if import -window root "$dest" 2>/dev/null; then
      echo "screenshot=${dest}"
      echo "screenshot_scope=root"
      return 0
    fi
  fi
  echo "screenshot=unavailable"
  return 1
}

ocr_contains() {
  local image="$1"
  local needle="$2"
  local key="$3"
  local text_file="$4"
  if [[ ! -f "$image" ]] || ! command -v tesseract >/dev/null 2>&1; then
    echo "${key}=no"
    return 0
  fi
  if [[ ! -f "$text_file" ]]; then
    tesseract "$image" stdout >"$text_file" 2>/dev/null || true
  fi
  if tr '\n' ' ' <"$text_file" | grep -q -F -- "$needle"; then
    echo "${key}=yes"
  else
    echo "${key}=no"
  fi
}

write_evidence() {
  local feature="$1"
  local body_file="$2"
  local evidence tmp
  mkdir -p "$EVIDENCE_DIR"
  evidence="${EVIDENCE_DIR}/${feature}.txt"
  tmp="$(mktemp "${EVIDENCE_DIR}/.${feature}.XXXXXX")"
  {
    echo "feature=${feature}"
    echo "command=bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh drive ${feature}"
    report_common
    cat "$body_file"
  } >"$tmp"
  mv "$tmp" "$evidence"
  cat "$evidence"
  echo "evidence=${evidence}"
}

thread_jsonl_count() {
  local dir="${CEDIA_HOME:-}/projects/${CEDIAN_PROJECT_SLUG:-}/threads"
  if [[ -d "$dir" ]]; then
    find "$dir" -name '*.jsonl' -type f | wc -l | tr -d ' '
  else
    echo 0
  fi
}

sample_cedia_argv() {
  local pid child args
  if [[ -f "$ARGV_FILE" ]] && grep -q -- '--print=' "$ARGV_FILE"; then
    return 0
  fi
  if ! pid="$(window_pid || true)" || [[ -z "$pid" ]]; then
    return 0
  fi
  for child in $(descendants "$pid"); do
    args="$(ps -o args= -p "$child" 2>/dev/null || true)"
    if [[ "$args" == *"--output-format"* ]]; then
      printf '%s\n' "$args" >>"$ARGV_FILE"
    fi
  done
  if [[ -n "${FAKE_THREADS:-}" && -f "${FAKE_THREADS}/../args" ]]; then
    echo "fake_args_file=${FAKE_THREADS}/../args" >>"$ARGV_FILE"
    cat "${FAKE_THREADS}/../args" >>"$ARGV_FILE"
  fi
}

daemon_round() {
  python3 - "$@" <<'PY'
import json, os, socket, sys
payload = json.loads(sys.argv[1])
want = sys.argv[2]
sock_path = os.environ["CEDIAN_STATE_DIR"] + "/daemon.sock"
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.connect(sock_path)
s.settimeout(0.3)
buf = b""
def pull(limit):
    global buf
    end = __import__("time").time() + limit
    notes = []
    while __import__("time").time() < end:
        try:
            chunk = s.recv(65536)
        except socket.timeout:
            continue
        if not chunk:
            break
        buf += chunk
        while b"\n" in buf:
            line, buf = buf.split(b"\n", 1)
            if line:
                notes.append(json.loads(line))
    return notes
pull(0.6)
s.settimeout(0.4)
s.sendall((json.dumps(payload) + "\n").encode())
notes = pull(5)
if want == "theme":
    for note in notes:
        desktop = note.get("desktop") or {}
        if note.get("kind") == "settings" and desktop.get("theme") == "light":
            print("desktop_theme=light")
            raise SystemExit(0)
    print("desktop_theme=unread")
    raise SystemExit(1)
print("prompt=sent")
PY
}

send_prompt() {
  local payload
  if [[ "${CEDIAN_CEDIA_KIND:-}" == pinned || "${CEDIAN_CEDIA_KIND:-}" == override ]]; then
    payload='{"op":"prompt","cwd":"'"$CEDIAN_PROJECT"'","text":"make f.txt say good","worktree":false,"model":"faux"}'
  else
    payload='{"op":"prompt","cwd":"'"$CEDIAN_PROJECT"'","text":"make f.txt say good","worktree":false}'
  fi
  daemon_round "$payload" prompt
}

set_theme() {
  daemon_round '{"op":"set_desktop","key":"theme","value":"light"}' theme
}

send_block_key() {
  local key="$1"
  local id=""
  if [[ -f "$WINDOW_ID_FILE" ]]; then
    id="$(tr -d '[:space:]' <"$WINDOW_ID_FILE")"
  fi
  if [[ -z "$id" ]] || ! command -v xdotool >/dev/null 2>&1; then
    echo "keys_sent=0"
    echo "key=${key}"
    return 1
  fi
  xdotool windowactivate --sync "$id" >/dev/null 2>&1 || true
  xdotool key --window "$id" --delay 30 "$key" >/dev/null
  echo "keys_sent=1"
  echo "key=${key}"
}

cmd_drive() {
  local feature="${1:-}"
  local body pid exit_code
  case "$feature" in
    agent-pane|editor-pane|spawn-cedia|terminal-pane|review-pane|desktop-settings|composer|sidebar|project-tab) ;;
    *)
      echo "usage: verify-cedian.sh drive <${FEATURES}>" >&2
      exit 2
      ;;
  esac
  refuse_user_state "$DATA_DIR"
  refuse_user_state "$EVIDENCE_DIR"
  body="$(mktemp)"
  exit_code=1
  if pid="$(window_pid || true)" && [[ -n "${pid}" ]]; then
    echo "window=started" >>"$body"
    echo "pid=${pid}" >>"$body"
  else
    echo "window=not-started" >>"$body"
    pid=""
  fi
  case "$feature" in
    agent-pane)
      local shot ocr i
      shot="${EVIDENCE_DIR}/agent-pane.png"
      ocr="${EVIDENCE_DIR}/agent-pane.ocr.txt"
      rm -f "$ocr" "$ARGV_FILE"
      (
        for ((n = 0; n < 200; n++)); do
          sample_cedia_argv || true
          if [[ -f "$ARGV_FILE" ]] && grep -q -- '--print=' "$ARGV_FILE"; then
            exit 0
          fi
          sleep 0.05
        done
      ) &
      local sampler=$!
      send_prompt >>"$body"
      wait "$sampler" || true
      for ((i = 0; i < 80; i++)); do
        if [[ "$(thread_jsonl_count)" != 0 ]]; then
          break
        fi
        sleep 0.25
      done
      sleep 1
      capture_window "$shot" >>"$body" || true
      ocr_contains "$shot" "say good" "ocr_prompt" "$ocr" >>"$body"
      echo "thread_files=$(thread_jsonl_count)" >>"$body"
      if [[ "$(thread_jsonl_count)" != 0 ]]; then
        echo "thread_file=present" >>"$body"
      else
        echo "thread_file=absent" >>"$body"
      fi
      if grep -q '^window=started$' "$body" \
        && grep -q '^ocr_prompt=yes$' "$body" \
        && grep -q '^thread_file=present$' "$body" \
        && [[ "$(home_cedia_state)" == absent ]]; then
        exit_code=0
      fi
      ;;
    editor-pane)
      local shot ocr
      shot="${EVIDENCE_DIR}/editor-pane.png"
      ocr="${EVIDENCE_DIR}/editor-pane.ocr.txt"
      rm -f "$ocr"
      send_block_key "ctrl+alt+shift+e" >>"$body" || true
      sleep 0.8
      capture_window "$shot" >>"$body" || true
      ocr_contains "$shot" "Editor" "ocr_editor" "$ocr" >>"$body"
      if grep -q '^window=started$' "$body" && grep -q '^ocr_editor=yes$' "$body"; then
        exit_code=0
      fi
      ;;
    terminal-pane)
      local shot child comm
      shot="${EVIDENCE_DIR}/terminal-pane.png"
      : >"$SHELL_PID_FILE"
      send_block_key "ctrl+alt+shift+t" >>"$body" || true
      sleep 0.8
      capture_window "$shot" >>"$body" || true
      if [[ -n "$pid" ]]; then
        for child in $(descendants "$pid"); do
          comm="$(ps -o comm= -p "$child" 2>/dev/null | tr -d ' ' || true)"
          args="$(ps -o args= -p "$child" 2>/dev/null || true)"
          case "$args" in
            *cedian-daemon*|*fake/cedia*|*src/cli.ts*) continue ;;
          esac
          case "$comm" in
            bash|zsh|sh|fish|dash)
              echo "$child" >>"$SHELL_PID_FILE"
              echo "shell_pid=${child}" >>"$body"
              echo "shell_comm=${comm}" >>"$body"
              ;;
          esac
        done
      fi
      if [[ -s "$SHELL_PID_FILE" ]]; then
        echo "shell_process=present" >>"$body"
        exit_code=0
      else
        echo "shell_process=absent" >>"$body"
      fi
      ;;
    spawn-cedia)
      sample_cedia_argv || true
      echo "thread_files=$(thread_jsonl_count)" >>"$body"
      if [[ -s "$ARGV_FILE" ]] && grep -q -- '--print=' "$ARGV_FILE" && grep -q -- 'stream-json' "$ARGV_FILE"; then
        echo "cedia_argv=print-stream-json" >>"$body"
      else
        echo "cedia_argv=missing" >>"$body"
      fi
      if [[ "$(thread_jsonl_count)" != 0 ]]; then
        echo "thread_file=present" >>"$body"
      else
        echo "thread_file=absent" >>"$body"
      fi
      if grep -q '^cedia_argv=print-stream-json$' "$body" \
        && grep -q '^thread_file=present$' "$body" \
        && [[ "$(home_cedia_state)" == absent ]]; then
        exit_code=0
      fi
      ;;
    review-pane)
      local shot ocr
      shot="${EVIDENCE_DIR}/review-pane.png"
      ocr="${EVIDENCE_DIR}/review-pane.ocr.txt"
      rm -f "$ocr"
      capture_window "$shot" >>"$body" || true
      ocr_contains "$shot" "snapshot" "ocr_snapshot" "$ocr" >>"$body"
      ocr_contains "$shot" "git HEAD" "ocr_baseline" "$ocr" >>"$body"
      if grep -q '^ocr_snapshot=yes$' "$body" && grep -q '^ocr_baseline=yes$' "$body"; then
        echo "review_pane=shown" >>"$body"
        exit_code=0
      else
        echo "review_pane=absent" >>"$body"
      fi
      ;;
    desktop-settings)
      local settings i
      settings="${CEDIA_HOME}/settings.json"
      for ((i = 0; i < 40; i++)); do
        if [[ -f "$settings" ]] && grep -q '"layout"' "$settings"; then
          break
        fi
        sleep 0.25
      done
      if set_theme >>"$body"; then
        :
      else
        echo "desktop_theme=unread" >>"$body"
      fi
      if [[ -f "$settings" ]]; then
        echo "settings_file=present" >>"$body"
        echo "settings_path=${settings}" >>"$body"
      else
        echo "settings_file=absent" >>"$body"
      fi
      if [[ -f "$settings" ]] && python3 - "$settings" >>"$body" <<'PY'
import json, sys
file = json.load(open(sys.argv[1]))
desktop = file.get("desktop") or {}
print("sandbox=%s" % ("kept" if (file.get("sandbox") or {}).get("mode") == "off" else "lost"))
print("layout=%s" % ("present" if "layout" in desktop else "absent"))
print("theme=%s" % desktop.get("theme", "absent"))
PY
      then
        :
      fi
      if grep -q '^desktop_theme=light$' "$body" \
        && grep -q '^sandbox=kept$' "$body" \
        && grep -q '^layout=present$' "$body" \
        && [[ "$(home_cedia_state)" == absent ]]; then
        exit_code=0
      fi
      ;;
    composer)
      local shot ocr
      shot="${EVIDENCE_DIR}/composer.png"
      ocr="${EVIDENCE_DIR}/composer.ocr.txt"
      rm -f "$ocr"
      capture_window "$shot" >>"$body" || true
      ocr_contains "$shot" "worktree" "ocr_worktree" "$ocr" >>"$body"
      ocr_contains "$shot" "Enter" "ocr_enter" "$ocr" >>"$body"
      if grep -q '^ocr_worktree=yes$' "$body" && grep -q '^ocr_enter=yes$' "$body"; then
        exit_code=0
      fi
      ;;
    sidebar)
      local shot ocr
      shot="${EVIDENCE_DIR}/sidebar.png"
      ocr="${EVIDENCE_DIR}/sidebar.ocr.txt"
      rm -f "$ocr"
      capture_window "$shot" >>"$body" || true
      ocr_contains "$shot" "Threads" "ocr_threads" "$ocr" >>"$body"
      if grep -q '^ocr_threads=yes$' "$body"; then
        exit_code=0
      fi
      ;;
    project-tab)
      local name
      name="$(window_name)"
      echo "window_name=${name}" >>"$body"
      if [[ "$name" == "project" ]]; then
        exit_code=0
      fi
      ;;
  esac
  if ! print_ready >>"$body"; then
    exit_code=1
  fi
  echo "exit=${exit_code}" >>"$body"
  write_evidence "$feature" "$body"
  rm -f "$body"
  exit "$exit_code"
}

shell_after_quit() {
  local pid alive=0
  if [[ ! -s "$SHELL_PID_FILE" ]]; then
    echo "shell_after_quit=no-shell"
    return 0
  fi
  while read -r pid; do
    [[ -z "$pid" ]] && continue
    if kill -0 "$pid" 2>/dev/null; then
      alive=1
      echo "shell_still_alive=${pid}"
    fi
  done <"$SHELL_PID_FILE"
  if [[ "$alive" == 1 ]]; then
    echo "shell_after_quit=alive"
    while read -r pid; do
      stop_pid "$pid"
    done <"$SHELL_PID_FILE"
  else
    echo "shell_after_quit=dead"
  fi
  if [[ -f "${EVIDENCE_DIR}/terminal-pane.txt" ]]; then
    echo "shell_after_quit=$([[ "$alive" == 1 ]] && echo alive || echo dead)" >>"${EVIDENCE_DIR}/terminal-pane.txt"
  fi
}

cmd_cleanup() {
  local pid note daemon
  note="$(mktemp)"
  if pid="$(window_pid || true)" && [[ -n "$pid" ]]; then
    stop_pid "$pid"
    echo "killed=${pid}" >>"$note"
  else
    echo "killed=none" >>"$note"
  fi
  rm -f "$PID_FILE"
  if [[ -f "$DAEMON_PID_FILE" ]]; then
    daemon="$(tr -d '[:space:]' <"$DAEMON_PID_FILE")"
    stop_pid "$daemon"
    echo "daemon_killed=${daemon}" >>"$note"
    rm -f "$DAEMON_PID_FILE"
  else
    echo "daemon_killed=none" >>"$note"
  fi
  shell_after_quit >>"$note"
  if [[ -d "$DATA_DIR" ]]; then
    refuse_user_state "$DATA_DIR"
    rm -rf "$DATA_DIR"
  fi
  if [[ -d "$LIVE_DIR" ]]; then
    refuse_user_state "$LIVE_DIR"
    rm -rf "$LIVE_DIR"
  fi
  rm -f "$LIVE_ENV"
  echo "data_dir_removed=${DATA_DIR}" >>"$note"
  echo "live_dir_removed=${LIVE_DIR}" >>"$note"
  echo "evidence_dir=${EVIDENCE_DIR}" >>"$note"
  if [[ -d "$EVIDENCE_DIR" ]]; then
    find "$EVIDENCE_DIR" -maxdepth 1 -type f -print >>"$note"
  fi
  echo "home_cedia=$(home_cedia_state)" >>"$note"
  cat "$note"
  rm -f "$note"
}

usage() {
  echo "usage: verify-cedian.sh <launch|doctor|drive|cleanup>" >&2
  exit 2
}

main() {
  local sub="${1:-}"
  shift || true
  load_live
  case "$sub" in
    launch) cmd_launch "$@" ;;
    doctor) cmd_doctor "$@" ;;
    drive) cmd_drive "$@" ;;
    cleanup) cmd_cleanup "$@" ;;
    *) usage ;;
  esac
}

main "$@"
