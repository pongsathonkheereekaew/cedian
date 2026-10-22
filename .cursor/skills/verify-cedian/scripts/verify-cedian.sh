#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../../.." && pwd)"
DATA_DIR="${CEDIAN_DATA_DIR:-/tmp/verify-cedian/data}"
EVIDENCE_DIR="${CEDIAN_EVIDENCE_DIR:-/tmp/verify-cedian/evidence}"
CEDIA_DIR="${CEDIA_CHECKOUT:-/home/ubuntu/cedia}"
PID_FILE="${CEDIAN_PID_FILE:-/tmp/verify-cedian/cedian.pid}"

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
  if command -v cedian >/dev/null 2>&1; then
    command -v cedian
  fi
}

report_common() {
  local bin cargo cedia_state entries
  bin="$(cedian_bin_path || true)"
  if [[ -f "$ROOT/Cargo.toml" ]]; then
    cargo=present
  else
    cargo=missing
  fi
  if git -C "$CEDIA_DIR" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    cedia_state=present
  elif [[ -e "$CEDIA_DIR" ]]; then
    cedia_state=not-a-git-checkout
  else
    cedia_state=missing
  fi
  echo "cedian_bin=${bin}"
  echo "cargo_toml=${cargo}"
  echo "cedia_checkout=${CEDIA_DIR}"
  echo "cedia_checkout_state=${cedia_state}"
  echo "data_dir=${DATA_DIR}"
  if [[ -d "$DATA_DIR" ]]; then
    entries="$(find "$DATA_DIR" -mindepth 1 -maxdepth 1 | wc -l | tr -d ' ')"
    echo "data_dir_entries=${entries}"
  else
    echo "data_dir_entries=missing"
  fi
  echo "home_cedia=$(home_cedia_state)"
  if [[ -n "$bin" && "$cargo" == present ]]; then
    echo "ready=no"
    echo "reason=a cedian binary is on PATH but this repo has no data-dir flag to keep it off ~/.cedia"
  elif [[ -z "$bin" && "$cargo" == missing ]]; then
    echo "ready=no"
    echo "reason=Cargo.toml is missing and no cedian binary is on PATH"
  else
    echo "ready=no"
    echo "reason=cedian is not runnable from this checkout"
  fi
}

cmd_launch() {
  refuse_user_state "$DATA_DIR"
  rm -rf "$DATA_DIR"
  mkdir -p "$DATA_DIR"
  echo "command=launch"
  report_common
  echo "pid_file=none"
  exit 1
}

cmd_doctor() {
  refuse_user_state "$DATA_DIR"
  echo "command=doctor"
  report_common
  exit 1
}

cmd_drive() {
  local feature="${1:-}"
  local evidence tmp
  case "$feature" in
    agent-pane|spawn-cedia|terminal-pane|review-pane|desktop-settings) ;;
    *)
      echo "usage: verify-cedian.sh drive <agent-pane|spawn-cedia|terminal-pane|review-pane|desktop-settings>" >&2
      exit 2
      ;;
  esac
  refuse_user_state "$DATA_DIR"
  refuse_user_state "$EVIDENCE_DIR"
  mkdir -p "$EVIDENCE_DIR"
  evidence="${EVIDENCE_DIR}/${feature}.txt"
  tmp="$(mktemp "${EVIDENCE_DIR}/.${feature}.XXXXXX")"
  {
    echo "feature=${feature}"
    echo "command=bash .cursor/skills/verify-cedian/scripts/verify-cedian.sh drive ${feature}"
    report_common
    case "$feature" in
      spawn-cedia)
        if [[ -f "${CEDIA_DIR}/bin/cedia" ]]; then
          echo "cedia_entrypoint=present"
        else
          echo "cedia_entrypoint=missing"
        fi
        echo "child_process=not-started"
        echo "thread_file=absent"
        ;;
      desktop-settings)
        if [[ "$(home_cedia_state)" == absent ]]; then
          echo "settings_file=absent"
        else
          echo "settings_file=present-not-read"
        fi
        echo "desktop_key=unread"
        ;;
      agent-pane|terminal-pane|review-pane)
        echo "window=not-started"
        ;;
    esac
    echo "exit=1"
  } >"$tmp"
  mv "$tmp" "$evidence"
  cat "$evidence"
  echo "evidence=${evidence}"
  exit 1
}

cmd_cleanup() {
  local pid
  if [[ -f "$PID_FILE" ]]; then
    pid="$(tr -d '[:space:]' <"$PID_FILE")"
    if [[ "$pid" =~ ^[0-9]+$ ]] && kill -0 "$pid" 2>/dev/null; then
      kill "$pid"
      echo "killed=${pid}"
    else
      echo "killed=none"
    fi
    rm -f "$PID_FILE"
  else
    echo "killed=none"
  fi
  if [[ -d "$DATA_DIR" ]]; then
    refuse_user_state "$DATA_DIR"
    rm -rf "$DATA_DIR"
  fi
  echo "data_dir_removed=${DATA_DIR}"
  echo "evidence_dir=${EVIDENCE_DIR}"
  if [[ -d "$EVIDENCE_DIR" ]]; then
    find "$EVIDENCE_DIR" -maxdepth 1 -type f -print
  fi
  echo "home_cedia=$(home_cedia_state)"
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
