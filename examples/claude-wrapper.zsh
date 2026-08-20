# Recommended shell wrapper (README "Install"): `claude` inside a herdr
# pane launches as a teammux lead automatically; plain `claude` everywhere
# else; `command claude` bypasses. Source from ~/.zshrc or copy the
# function in.
claude() {
  if [[ -n "$HERDR_PANE_ID" ]] && command -v herdmates >/dev/null 2>&1; then
    herdmates teammux-launch "$@"
  else
    command claude "$@"
  fi
}
