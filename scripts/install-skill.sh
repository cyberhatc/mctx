#!/usr/bin/env bash
# Install the mctx skill globally for opencode and Claude Code / other agents.
#
#   bash scripts/install-skill.sh
#
# Copies skills/mctx into every agent skill directory it finds:
#   ~/.config/opencode/skills/mctx     (opencode global skills)
#   ~/.claude/skills/mctx              (Claude Code, auto-loaded externally)
#   ~/.agents/skills/mctx              (generic agents, auto-loaded)
set -euo pipefail

SRC="$(cd "$(dirname "$0")/../skills/mctx" && pwd)"

install_to() {
  local dir="$1"
  [ -n "${dir:-}" ] || return 0
  mkdir -p "$dir"
  rm -rf "$dir/mctx"
  cp -r "$SRC" "$dir/mctx"
  echo "[mctx] skill installed -> $dir/mctx"
}

if command -v opencode >/dev/null 2>&1; then
  install_to "${OPENCODE_CONFIG_DIR:-$HOME/.config/opencode}/skills"
fi
install_to "${CLAUDE_CONFIG_DIR:-$HOME/.claude}/skills"
install_to "$HOME/.agents/skills"

echo "[mctx] done. Restart opencode / the agent for the skill to load."
