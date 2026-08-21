#!/usr/bin/env bash
# Boundary-crossing checks for issue #127's seam inventory
# (docs/agents/boundary-pairs.md). Mechanical checks only — most seams in
# that doc are marked "manual — reviewer brief only" because there's no
# shared vocabulary a script could line up without becoming a
# hand-maintained duplicate of the thing it's checking.
#
# ADR-0004 (cd-in-prompt-text): "Pane cwd is ALWAYS set at creation (herdr
# --cwd), never via prompt text" — a prompt-level `cd` causes split-brain
# (relative writes leak into the launch dir). Production code must never
# build a `cd <path> && ...` command string. Test fixtures are exempt: they
# legitimately echo Claude Code's OWN tmux calls (teammux.rs's respawn_pane
# tests record what the external process sends, not what herdmates
# constructs), so this check strips everything from `#[cfg(test)]` to EOF
# (this codebase's convention: tests live in a trailing `mod tests` block)
# and skips comment lines before grepping.
#
# Output is agent-readable: [TAG] / File: / FIX:
# Exit 0 = clean, 1 = drift found.
set -uo pipefail
cd "$(dirname -- "$0")/.." || exit 1

fail=0
report() { printf '[%s]\nFile: %s\nFIX: %s\n\n' "$1" "$2" "$3"; fail=1; }

for f in src/*.rs; do
  # Production code only: cut everything from the first #[cfg(test)] line
  # onward, then drop comment lines, before searching for the anti-pattern.
  hits=$(awk '/^#\[cfg\(test\)\]/ { exit } { print }' "$f" \
    | grep -vE '^\s*//' \
    | grep -noE 'cd [^"]{1,80}&&' || true)
  if [ -n "$hits" ]; then
    while IFS= read -r hit; do
      [ -n "$hit" ] || continue
      line=${hit%%:*}
      snippet=${hit#*:}
      report "SEAM-CD-IN-PROMPT" "$f:$line" \
        "Found \"cd ... &&\" ($snippet) built into a command string — ADR-0004 requires pane cwd be set via herdr --cwd at pane creation, never via prompt text. Set cwd on pane_split/workspace_create instead."
    done <<< "$hits"
  fi
done

if [ "$fail" -eq 0 ]; then
  echo "OK: boundary seams agree (ADR-0004 cd-in-prompt-text)"
fi
exit "$fail"
