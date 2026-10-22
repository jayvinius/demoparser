# PugStar fork notes

Local branch `pugstar/subtick`, based on upstream `main` at 45ca85a (after v0.42.01).
Not pushed anywhere. PugStar builds the Python package from this tree.

## Why not PyPI 0.42.0

Since the July 2026 CS2 update, servers write about 99.8% of user commands as
`CMsgServerUserCmd.delta_data`. With 0.42.0, 40 to 70% of input-history view angles on
those demos come back zeroed, and 20 to 25% of the non-zero ones are wrong. Upstream
`main` already fixes this (#352, sparse repeated deltas) but it isn't released yet. On five
test demos, upstream `main` decodes 100% of deltas with history angles that pass PugStar's
plausibility check 97 to 98% of the time, matching pre-July demos.

## Changes on this branch

- **Exposes the click sample.** `usercmd_attack1_start_history_index` and
  `usercmd_attack2_start_history_index` are now props. The first one is the index into
  `usercmd_input_history` of the frame where the shot was clicked. It's present on the
  `weapon_fire` tick for 98 to 99% of first shots.
- **Fuller input-history entries.** Entries now include `frame_number`,
  `target_ent_index`, `shoot_position`, `target_head_pos_check`, `target_abs_pos_check`
  and `target_abs_ang_check`. In the GOTV demos tested, Valve strips all of these (they
  come back empty or default), but they're decoded correctly if a demo source ever has them.
- **Delta decoder schemas.** Adds schemas for `CMsgVector` and the interpolation-info
  messages. Before this, a wire-type-7 reset inside `shoot_position`, the target checks or
  `cl_interp`/`sv_interp*` failed the whole command, and every later delta was then built
  on a stale baseline.
- **Exact reset semantics.** A reset of a field on an input-history or subtick element now
  clears it to `None`, matching demoinfocs-golang's `Clear()`. Writing a default nested
  message used to leave the old values in place.
- **Error reasons.** `apply_delta` returns `Result<_, DeltaError>` with a reason instead
  of `Option`.
- **Decode counters.** `second_pass::usercmd_audit` counts full vs delta commands,
  failures by reason, duplicate and out-of-order command numbers, and history entries with
  missing angles. Run the `usercmd_audit` binary on a demo to print them.
- **No codegen by default.** `csgoproto/build.rs` now uses the committed `src/protobuf.rs`
  unless `CSGOPROTO_REGENERATE=1` is set. That file carries hand edits, and regenerating it
  from the newest upstream protos fails with protoc 36 anyway.

## Build

```sh
# Rust tests and the audit tool
cd src/parser
cargo test --release --lib
cargo run --release --bin usercmd_audit -- path/to/demo.dem

# Python package into PugStar's environment (PugStar's pyproject points here)
cd <pugstar>/services/analyzer && uv sync
```

After changing Rust code, bump the local version in `src/python/Cargo.toml`
(`0.42.0+pugstar.N`) so uv rebuilds instead of reusing its cached wheel.
