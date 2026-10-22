#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../../.." && pwd)"
DATA_DIR="${CEDIAN_DATA_DIR:-/tmp/verify-cedian/data}"
EVIDENCE_DIR="${CEDIAN_EVIDENCE_DIR:-/tmp/verify-cedian/evidence}"
CEDIA_DIR="${CEDIA_CHECKOUT:-/home/ubuntu/cedia}"
PID_FILE="${CEDIAN_PID_FILE:-/tmp/verify-cedian/cedian.pid}"
REPLAY="${CEDIAN_REPLAY:-$ROOT/crates/cedian_protocol/cedia/sessions/edit-and-check/stream.jsonl}"
WINDOW_ID_FILE="${EVIDENCE_DIR}/window.id"
SHELL_PID_FILE="${EVIDENCE_DIR}/shell.pids"
LOG_FILE="${EVIDENCE_DIR}/cedian.log"

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

cedia_checkout_state() {
  if git -C "$CEDIA_DIR" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    echo present
  elif [[ -e "$CEDIA_DIR" ]]; then
    echo not-a-git-checkout
  else
    echo missing
  fi
}

data_dir_entries() {
  if [[ -d "$DATA_DIR" ]]; then
    find "$DATA_DIR" -mindepth 1 -maxdepth 1 | wc -l | tr -d ' '
  else
    echo missing
  fi
}

report_common() {
  local bin cargo
  bin="$(cedian_bin_path || true)"
  if [[ -f "$ROOT/Cargo.toml" ]]; then
    cargo=present
  else
    cargo=missing
  fi
  echo "cedian_bin=${bin}"
  echo "cargo_toml=${cargo}"
  echo "cedia_checkout=${CEDIA_DIR}"
  echo "cedia_checkout_state=$(cedia_checkout_state)"
  echo "data_dir=${DATA_DIR}"
  echo "data_dir_entries=$(data_dir_entries)"
  echo "home_cedia=$(home_cedia_state)"
  echo "replay=${REPLAY}"
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
  echo "ready=yes"
  echo "pid=${pid}"
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
  xwininfo -root -tree 2>/dev/null | awk '/1200x760\+/ { print $1 }' | sort -u
}

remember_window() {
  local before="$1"
  local id
  id="$(comm -13 "$before" <(content_window_ids) | head -n 1 || true)"
  if [[ -z "$id" ]]; then
    echo "window_id=none"
    rm -f "$WINDOW_ID_FILE"
    return 0
  fi
  echo "$id" >"$WINDOW_ID_FILE"
  echo "window_id=${id}"
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

cmd_launch() {
  local bin before pid
  refuse_user_state "$DATA_DIR"
  refuse_user_state "$EVIDENCE_DIR"
  if pid="$(window_pid)"; then
    stop_pid "$pid"
  fi
  rm -f "$PID_FILE"
  rm -rf "$DATA_DIR"
  mkdir -p "$DATA_DIR" "$EVIDENCE_DIR"
  echo "command=launch"
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
  if [[ ! -f "$REPLAY" ]]; then
    echo "ready=no"
    echo "reason=replay fixture is missing"
    echo "pid_file=none"
    exit 1
  fi
  prepare_display
  echo "display=${DISPLAY}"
  before="$(mktemp)"
  content_window_ids >"$before"
  : >"$LOG_FILE"
  nohup "$bin" --replay "$REPLAY" >"$LOG_FILE" 2>&1 </dev/null &
  pid="$!"
  echo "$pid" >"$PID_FILE"
  local i
  for ((i = 0; i < 45; i++)); do
    if ! kill -0 "$pid" 2>/dev/null; then
      echo "ready=no"
      echo "reason=cedian exited during launch"
      echo "pid=${pid}"
      echo "log=${LOG_FILE}"
      rm -f "$before"
      exit 1
    fi
    if ((i >= 30)); then
      break
    fi
    sleep 1
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
  echo "ready=yes"
  echo "pid=${pid}"
  echo "pid_file=${PID_FILE}"
  echo "log=${LOG_FILE}"
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

thread_files() {
  if [[ -d "$DATA_DIR" ]]; then
    find "$DATA_DIR" -name '*.jsonl' -type f | wc -l | tr -d ' '
  else
    echo 0
  fi
}

cmd_drive() {
  local feature="${1:-}"
  local body pid exit_code
  case "$feature" in
    agent-pane|editor-pane|spawn-cedia|terminal-pane|review-pane|desktop-settings) ;;
    *)
      echo "usage: verify-cedian.sh drive <agent-pane|editor-pane|spawn-cedia|terminal-pane|review-pane|desktop-settings>" >&2
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
      local shot ocr
      shot="${EVIDENCE_DIR}/agent-pane.png"
      ocr="${EVIDENCE_DIR}/agent-pane.ocr.txt"
      rm -f "$ocr"
      capture_window "$shot" >>"$body" || true
      ocr_contains "$shot" "say good" "ocr_prompt" "$ocr" >>"$body"
      ocr_contains "$shot" "verified" "ocr_verified" "$ocr" >>"$body"
      if grep -q '^ocr_prompt=yes$' "$body" || grep -q '^ocr_verified=yes$' "$body"; then
        echo "ocr_blocks=yes" >>"$body"
      else
        echo "ocr_blocks=no" >>"$body"
      fi
      echo "thread_files=$(thread_files)" >>"$body"
      if [[ "$(thread_files)" == 0 ]]; then
        echo "thread_file=absent" >>"$body"
      else
        echo "thread_file=present" >>"$body"
      fi
      echo "agent_state=not-shown" >>"$body"
      if grep -q '^window=started$' "$body" \
        && grep -q '^ocr_blocks=yes$' "$body" \
        && grep -q '^thread_file=absent$' "$body" \
        && [[ "$(home_cedia_state)" == absent ]]; then
        exit_code=0
      fi
      ;;
    editor-pane)
      local shot ocr
      shot="${EVIDENCE_DIR}/editor-pane.png"
      ocr="${EVIDENCE_DIR}/editor-pane.ocr.txt"
      rm -f "$ocr"
      capture_window "$shot" >>"$body" || true
      ocr_contains "$shot" "cedian spike" "ocr_editor" "$ocr" >>"$body"
      if grep -q '^window=started$' "$body" && grep -q '^ocr_editor=yes$' "$body"; then
        exit_code=0
      fi
      ;;
    terminal-pane)
      local shot child comm
      shot="${EVIDENCE_DIR}/terminal-pane.png"
      capture_window "$shot" >>"$body" || true
      : >"$SHELL_PID_FILE"
      echo "keys_sent=0" >>"$body"
      if [[ -n "$pid" ]]; then
        for child in $(descendants "$pid"); do
          comm="$(ps -o comm= -p "$child" 2>/dev/null | tr -d ' ' || true)"
          echo "descendant=${child} ${comm}" >>"$body"
          case "$comm" in
            bash|zsh|sh|fish|dash)
              echo "$child" >>"$SHELL_PID_FILE"
              ;;
          esac
        done
      fi
      if [[ -s "$SHELL_PID_FILE" ]]; then
        echo "shell_process=present" >>"$body"
        echo "shell_pids=$(paste -sd, "$SHELL_PID_FILE")" >>"$body"
        exit_code=0
      else
        echo "shell_process=absent" >>"$body"
      fi
      ;;
    spawn-cedia)
      local child comm found
      found=0
      if [[ -f "${CEDIA_DIR}/bin/cedia" || -x "${CEDIA_DIR}/bin/cedia" ]]; then
        echo "cedia_entrypoint=present" >>"$body"
      else
        echo "cedia_entrypoint=missing" >>"$body"
      fi
      if [[ -n "$pid" ]]; then
        for child in $(descendants "$pid"); do
          comm="$(ps -o comm= -p "$child" 2>/dev/null | tr -d ' ' || true)"
          echo "descendant=${child} ${comm}" >>"$body"
          if [[ "$comm" == cedia ]]; then
            found=1
          fi
        done
      fi
      if [[ "$found" == 1 ]]; then
        echo "child_process=started" >>"$body"
      else
        echo "child_process=not-started" >>"$body"
      fi
      if [[ "$(thread_files)" != 0 ]]; then
        echo "thread_file=present" >>"$body"
      else
        echo "thread_file=absent" >>"$body"
      fi
      echo "thread_files=$(thread_files)" >>"$body"
      if [[ "$found" == 1 && "$(thread_files)" != 0 && "$(home_cedia_state)" == absent ]]; then
        exit_code=0
      fi
      ;;
    review-pane)
      echo "review_pane=absent" >>"$body"
      echo "baseline=absent" >>"$body"
      ;;
    desktop-settings)
      if [[ -f "${DATA_DIR}/settings.json" || -f "${DATA_DIR}/.cedia/settings.json" ]]; then
        echo "settings_file=present" >>"$body"
      else
        echo "settings_file=absent" >>"$body"
      fi
      echo "desktop_key=unread" >>"$body"
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
    {
      echo "shell_after_quit=$([[ "$alive" == 1 ]] && echo alive || echo dead)"
    } >>"${EVIDENCE_DIR}/terminal-pane.txt"
  fi
}

cmd_cleanup() {
  local pid note
  note="$(mktemp)"
  if pid="$(window_pid || true)" && [[ -n "$pid" ]]; then
    stop_pid "$pid"
    echo "killed=${pid}" >>"$note"
  else
    echo "killed=none" >>"$note"
  fi
  rm -f "$PID_FILE"
  shell_after_quit >>"$note"
  if [[ -d "$DATA_DIR" ]]; then
    refuse_user_state "$DATA_DIR"
    rm -rf "$DATA_DIR"
  fi
  echo "data_dir_removed=${DATA_DIR}" >>"$note"
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
  case "$sub" in
    launch) cmd_launch "$@" ;;
    doctor) cmd_doctor "$@" ;;
    drive) cmd_drive "$@" ;;
    cleanup) cmd_cleanup "$@" ;;
    *) usage ;;
  esac
}

main "$@"
