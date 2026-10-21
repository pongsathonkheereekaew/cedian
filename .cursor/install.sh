#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [[ "$(id -u)" -eq 0 ]]; then
  as_root() { "$@"; }
else
  as_root() { sudo "$@"; }
fi

export DEBIAN_FRONTEND=noninteractive
pkgs=()
command -v curl >/dev/null 2>&1 || pkgs+=(curl)
command -v git >/dev/null 2>&1 || pkgs+=(git)
command -v cc >/dev/null 2>&1 || pkgs+=(build-essential)
command -v unzip >/dev/null 2>&1 || pkgs+=(unzip)
if ! dpkg -s ca-certificates >/dev/null 2>&1; then
  pkgs+=(ca-certificates)
fi
if ((${#pkgs[@]} > 0)); then
  as_root apt-get update
  as_root apt-get install -y --no-install-recommends "${pkgs[@]}"
fi

export PATH="/usr/local/bin:/usr/local/cargo/bin:${PATH}"

if ! command -v rustup >/dev/null 2>&1; then
  export RUSTUP_HOME="${HOME}/.rustup"
  export CARGO_HOME="${HOME}/.cargo"
  curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs \
    | sh -s -- -y --default-toolchain none --profile minimal --no-modify-path
  export PATH="${CARGO_HOME}/bin:${PATH}"
fi
cargo_bin_dir="$(dirname "$(command -v rustup)")"
for tool in rustup cargo rustc; do
  if [[ ! -e "/usr/local/bin/${tool}" ]]; then
    as_root ln -sfn "${cargo_bin_dir}/${tool}" "/usr/local/bin/${tool}"
  fi
done

channel="$(awk -F'"' '/^channel[[:space:]]*=/ { print $2; exit }' "$ROOT/rust-toolchain.toml")"
if [[ -z "${channel}" ]]; then
  echo "rust-toolchain.toml has no channel" >&2
  exit 1
fi
components="$(awk -F'[][]' '/^components[[:space:]]*=/ { print $2; exit }' "$ROOT/rust-toolchain.toml")"
components="${components// /}"
components="${components//\"/}"
if [[ -z "${components}" ]]; then
  echo "rust-toolchain.toml has no components" >&2
  exit 1
fi

rustup toolchain install "$channel" --profile minimal --component "$components"

if ! /usr/local/bin/bun --version >/dev/null 2>&1; then
  if ! "${HOME}/.bun/bin/bun" --version >/dev/null 2>&1; then
    curl -fsSL https://bun.sh/install | bash
  fi
  as_root ln -sfn "${HOME}/.bun/bin/bun" /usr/local/bin/bun
fi

(
  cd "$ROOT"
  echo "rustc=$(rustc --version)"
  echo "cargo=$(cargo --version)"
)
echo "bun=$(/usr/local/bin/bun --version)"
if command -v node >/dev/null 2>&1; then
  echo "node=$(node --version) path=$(command -v node)"
else
  echo "node=absent"
fi

CEDIA_DIR=/home/ubuntu/cedia
export GIT_TERMINAL_PROMPT=0
if git -C "$CEDIA_DIR" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  git -C "$CEDIA_DIR" fetch --prune origin
  branch="$(git -C "$CEDIA_DIR" symbolic-ref --short refs/remotes/origin/HEAD)"
  branch="${branch#origin/}"
  git -C "$CEDIA_DIR" checkout -B "$branch" "origin/$branch"
else
  rm -rf "$CEDIA_DIR"
  mkdir -p "$(dirname "$CEDIA_DIR")"
  git clone https://github.com/pongsathonkheereekaew/cedia.git "$CEDIA_DIR"
fi

if [[ -f "$CEDIA_DIR/bun.lock" || -f "$CEDIA_DIR/bun.lockb" ]]; then
  (cd "$CEDIA_DIR" && bun install --frozen-lockfile)
elif [[ -f "$CEDIA_DIR/package.json" ]]; then
  (cd "$CEDIA_DIR" && bun install)
else
  echo "cedia checkout has no package.json" >&2
  exit 1
fi

echo "cedia_checkout=${CEDIA_DIR}"
