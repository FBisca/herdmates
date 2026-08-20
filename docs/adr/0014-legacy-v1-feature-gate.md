# ADR-0014 — Fence the frozen legacy v1 surface behind a cargo feature

Date: 2026-08-20
Status: accepted
Builds on ADR-0012's legacy disposition (frozen at v1.1.0, no further
investment); does not change identity or scope, only how the freeze is
enforced. Caio approved 2026-08-20 (issue #116).

## Context

ADR-0012 froze the v1 orchestration surface (spawn/status/kill/msg/board/
control-deck) at v1.1.0: no further investment, removal deferred to a
2.0.0-scope decision. In practice the freeze has only ever been a doc
comment (`Frozen at v1.1.0 (ADR-0012): no further investment...` at the top
of each module). Nothing stops new code from depending on frozen internals,
and nothing shrinks the default build/lint/read surface — every `cargo
build`, `clippy`, and `cargo test` run still compiles and checks all ~11k
lines of frozen code alongside the live pivot surface.

A 2026-08-20 field audit (issue #116) was asked to recommend how to
structurally enforce the freeze without deleting code the pivot hasn't yet
proven unnecessary. Three options were on the table: (a) do nothing, keep
relying on doc comments; (b) a cargo feature gate; (c) split into a
workspace with a separate legacy crate. The audit recommended (b):
reversible (unlike delete), enforces the boundary at compile time instead of
by convention, shrinks the default build/lint/read surface, and defers the
irreversible delete until dogfooding produces evidence nobody needs the
legacy verbs. Workspace split (c) buys the same isolation at much higher
churn for no extra benefit at this scale.

## Decision

Add a cargo feature `legacy-v1` (empty, no dependencies) and gate every
module that serves only the frozen v1 surface behind
`#[cfg(feature = "legacy-v1")]`, both the `pub mod` declaration in
`src/lib.rs` and the corresponding subcommand arms in `src/main.rs`.

`herdmates` is a single crate producing both the `herdmates` binary and the
library `src/bin/teammux.rs` depends on — there is no per-target default
mechanism in Cargo, so a "default-off library, default-on binary" split (the
audit's literal phrasing) isn't achievable. Instead: **`legacy-v1` defaults
on** (`default = ["legacy-v1"]`) so `cargo install`, the published
`herdmates` binary, and `herdr-plugin.toml`'s legacy actions
(`spawn`/`status`/`kill`/`open-board`) are unchanged. The freeze is enforced
structurally by gating CI and local dev on `cargo build/clippy/test
--no-default-features` as a second, mandatory check — that's the
"frozen surface" verification; the default build stays a superset. This is
a deliberate deviation from the audit's default-off recommendation, driven
by the single-crate constraint rather than by preference.

### Gated modules (compile only with `legacy-v1`)

Two groups, both discovered by tracing actual `crate::` callers (not
guessed from doc comments):

**Explicitly marked frozen** (`Frozen at v1.1.0 (ADR-0012)` header,
dispatched from the legacy `main.rs` arms):
`adopt`, `board`, `god_cli`, `hook`, `msg`, `spawn`, `status_kill`.

**Unmarked but serve only frozen callers** (confirmed via `grep` for every
`crate::<module>::` call site — none of these have a caller outside the
list above):
`agents_md`, `launcher`, `reconcile`, `run`, `spec`, `types`.

**Surprise found during the audit, not named in the issue #116 recommendation:**
`socket` and `socket_backend` (the experimental protocol-16 NDJSON socket
transport, ADR-0011) are called only from `board.rs` and `god_cli.rs` — both
already frozen. Nothing on the live pivot surface (`pump`, `teammux`,
`recorder`, `pane_board`, `team_hook`, `doctor`, `signal_engine`) touches
the socket backend; it was built for the legacy control deck's optional
fast-path and never adopted elsewhere. Gated alongside the rest.

Also: two `herdr.rs` test-support helpers (`BriefOrder::new`,
`FakeHerdr::protocol_snapshots`) are used only by `spawn.rs`'s test suite.
`herdr` itself stays ungated (shared by live modules), so these two items
get `#[cfg_attr(not(feature = "legacy-v1"), allow(dead_code))]` instead of
a full module gate, to avoid an unused-function warning under
`--no-default-features`.

### Ungated (compiled in both configs)

`attention`, `audit`, `dateutil`, `doctor`, `focus_pane`, `focusfile`,
`gather`, `herdr`, `idmap`, `inbox_write`, `jump`, `metadata`, `pane_board`,
`paths`, `pump`, `recorder`, `signal_engine`, `teamfiles`, `team_hook`,
`teammux`, `teammux_launch`, `tmuxargs`, `tokens`. `herdr` and `metadata`
are called from both live and legacy modules and stay ungated per the
shared-caller rule.

### Deprecation markers

The eleven legacy subcommand entry points (`adopt_command`, `board_command`,
`open_report_command`, `spawn_command`, `status_command`, `kill_command`,
`inbox_command`, `report_command`, `wait_command`, `msg_command`,
`hook_command`) carry `#[deprecated(note = "frozen legacy v1 surface
(ADR-0012); native Claude Code teams replaced it")]`. Their own-crate call
sites (`main.rs` dispatch arms, three in-module test functions) carry
`#[allow(deprecated)]` rather than losing the marker — the point is to warn
external/future callers, not to silence the crate's own frozen internals
calling their own frozen entry points.

### CI

`.github/workflows/ci.yml` runs both configurations: the existing
`cargo fmt --check` / `cargo clippy --all-targets -- -D warnings` /
`cargo test` (default features, i.e. legacy-v1 on), plus
`cargo clippy --all-targets --no-default-features -- -D warnings` and
`cargo test --no-default-features`. The second pair is the actual freeze
gate — it fails if any live module accidentally grows a dependency on
frozen code.

## Consequences

- Default build, `cargo install`, and the installed `herdmates` binary are
  byte-for-byte unchanged in behavior — this is purely a structural fence,
  not a functional change. `./target/release/herdmates` with no args still
  lists all eighteen subcommands including the legacy ones.
- `cargo build/test/clippy --no-default-features` now compiles and tests
  only the live pivot surface: 316 of 496 tests run (180 legacy tests
  excluded), and the crate's public surface shrinks by 15 modules
  (`adopt`, `agents_md`, `board`, `god_cli`, `hook`, `launcher`, `msg`,
  `reconcile`, `run`, `socket`, `socket_backend`, `spawn`, `spec`,
  `status_kill`, `types`). Contributors working on the live pivot surface
  can develop against `--no-default-features` and never see frozen code in
  clippy output or `cargo doc`.
- Reversible in both directions: dropping the feature entirely and always
  compiling (option a) is a one-line Cargo.toml revert; deleting the gated
  modules outright (the eventual 2.0.0-scope decision ADR-0012 deferred) is
  now a smaller, well-scoped diff — remove the 15 `pub mod` lines, the
  `#[cfg]` main.rs arms, and the modules themselves, informed by whichever
  gated modules turn out to have sat completely unused for a release or two
  of dogfooding.
- **Delete criterion** (unchanged from ADR-0012, now with a concrete trigger):
  once `--no-default-features` has been the daily-driver config for a
  release or two and telemetry/dogfooding shows none of the eighteen legacy
  verbs (`adopt`/`board`/`open-report`/`spawn`/`status`/`kill`/`inbox`/
  `report`/`wait`/`msg`/`on-agent-status`) have been invoked, delete the
  fifteen gated modules and the `legacy-v1` feature outright. Until then the
  feature stays default-on so the shipped plugin loses nothing.

## Alternatives rejected

- **(a) Status quo (doc comments only):** what we had; doesn't stop new
  code from depending on frozen internals and never shrinks the default
  build surface. Rejected because it enforces nothing.
- **(c) Workspace split (separate legacy crate):** would give a cleaner
  default-off/default-on split (a binary crate could depend on the legacy
  crate only under a feature, achieving true per-target defaults) but costs
  a full workspace restructure — new `Cargo.toml` files, path deps,
  duplicated `[lints]`/CI matrix entries — for a freeze that's explicitly
  provisional pending dogfood evidence. Revisit if/when the delete decision
  is made and a legacy crate would instead need to be *kept* longer than
  expected.
