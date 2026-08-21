#!/usr/bin/env bash
# Boundary-crossing check for the subcommand seams (issue #127).
#
# The producers and consumers of the subcommand list live in three files that
# have to agree, and nothing enforced that agreement until now:
#
#   src/main.rs match arms   (the truth: what the binary actually dispatches)
#   src/main.rs help text    (what the binary tells a human it supports)
#   herdr-plugin.toml        (what herdr is told it may invoke)
#
# Both other sides drifted for real during the v3.0.0 wave (2026-08-20): the
# manifest kept advertising spawn/status/kill/board/open-report/on-agent-status
# for a whole commit after #119 deleted those arms, and the help string had to
# be hand-patched in #119 and again in #120. A cross-comparison of the two
# sides — not an existence check on one — is what catches this class.
#
# Output is agent-readable: [TAG] / File: / FIX:
# Exit 0 = seams agree, 1 = drift found.
set -uo pipefail
cd "$(dirname -- "$0")/.." || exit 1

fail=0
report() { printf '[%s]\nFile: %s\nFIX: %s\n\n' "$1" "$2" "$3"; fail=1; }

# --- Side A: live dispatch arms (the truth) --------------------------------
# String-literal match arms in main(), minus the help/fallback arms.
arms=$(grep -oE '^\s*"[a-z][a-z-]*" =>' src/main.rs \
  | grep -oE '"[a-z][a-z-]*"' | tr -d '"' | sort -u)
if [ -z "$arms" ]; then
  report "SEAM-PARSE" "src/main.rs" \
    "No subcommand match arms matched the parser. The dispatch shape changed — update this script's arm regex before trusting it."
  exit 1
fi

# --- Side B: the help/usage string -----------------------------------------
usage=$(grep -oE '"herdmates <[a-z|-]+>"' src/main.rs | head -1 \
  | sed -E 's/.*<([a-z|-]+)>.*/\1/' | tr '|' '\n' | sort -u)
if [ -z "$usage" ]; then
  report "SEAM-USAGE-MISSING" "src/main.rs" \
    "No 'herdmates <a|b|c>' usage string found. If the help text moved, point this check at its new home."
else
  only_arms=$(comm -23 <(echo "$arms") <(echo "$usage"))
  only_usage=$(comm -13 <(echo "$arms") <(echo "$usage"))
  [ -n "$only_arms" ] && report "SEAM-USAGE-STALE" "src/main.rs" \
    "Dispatched but undocumented in the usage string: $(echo "$only_arms" | tr '\n' ' ')— add them to the 'herdmates <...>' line."
  [ -n "$only_usage" ] && report "SEAM-USAGE-PHANTOM" "src/main.rs" \
    "Advertised in the usage string but not dispatched: $(echo "$only_usage" | tr '\n' ' ')— delete them from the 'herdmates <...>' line (this is the #119/#120 drift)."
fi

# --- Side C: the plugin manifest -------------------------------------------
# Any manifest command array invoking `herdmates <sub>` must name a live arm.
# Covers actions/panes/events/link_handlers alike — they all use command = [...].
while read -r sub; do
  [ -n "$sub" ] || continue
  echo "$arms" | grep -qx "$sub" || report "SEAM-MANIFEST-DEAD" "herdr-plugin.toml" \
    "Manifest invokes 'herdmates $sub', which main.rs does not dispatch. Delete the entry, or restore the subcommand (this is the #121 stale-wiring bug)."
done < <(grep -oE '"herdmates"[^]]*' herdr-plugin.toml \
  | grep -oE '"herdmates",\s*"[a-z][a-z-]*"' \
  | grep -oE '"[a-z][a-z-]*"$' | tr -d '"' | sort -u)

# --- Side D: in-code self-invocations --------------------------------------
# The binary re-spawns itself (pump.rs auto_pump: current_exe + "pump-board").
# A renamed arm silently disables that caller — stderr is nulled and the
# spawn Result discarded — so every literal .arg() within a few lines of a
# current_exe spawn must name a live arm.
while read -r sub; do
  [ -n "$sub" ] || continue
  echo "$arms" | grep -qx "$sub" || report "SEAM-SELFSPAWN-DEAD" "src (current_exe spawn)" \
    "Code spawns 'herdmates $sub' via current_exe, but main.rs does not dispatch it. Rename the .arg() literal to a live arm, or restore the subcommand."
done < <(awk '/current_exe/ { w = 10 }
  w > 0 { if (match($0, /\.arg\("[a-z][a-z-]*"\)/)) {
            s = substr($0, RSTART + 6, RLENGTH - 8); print s; w = 0 }
          w-- }' src/*.rs | sort -u)

if [ "$fail" -eq 0 ]; then
  echo "OK: subcommand seams agree — $(echo "$arms" | tr '\n' ' ' | sed 's/ $//')"
fi
exit "$fail"
