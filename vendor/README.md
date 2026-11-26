# Patched third-party crates

PingCraft builds these crates from here instead of crates.io, through `[patch.crates-io]` path entries in the
workspace `Cargo.toml`. Each folder is the crates.io release of that name and version, copied from cargo's registry
cache, unmodified except for the lines listed below. The folders are excluded from the workspace
(`[workspace] exclude`), so `cargo xtask ci`'s fmt, clippy and layers steps do not touch them; `cargo deny` checks
them like any other dependency (licence from their `Cargo.toml`, path source).

## Why

craft-libs' `craft-settings` depends on `craft-host`, which depends on `craft-i18n`. Its ICU crates turn on their
`serde` features, which turn on `zerovec`'s `serde` feature. With `zerovec/serde` in the build, rustc cannot infer
the type of a few `deserialize` calls in ruma whose type is left to inference, and fails with:

```text
error[E0275]: overflow evaluating the requirement `Box<_>: Deserialize<'_>`
```

(ruma-common 0.20.2 `src/room.rs`, then ruma-events 0.35.2 `src/relation/rel_serde.rs`). With its `serde` and
`alloc` features, zerovec has `impl Deserialize for Box<VarZeroSlice<T, F>> where Box<T>: Deserialize`, a recursive
candidate for `Box<_>`; with the type left open the solver recurses through it until it hits the limit. Writing the
type out removes the search; behaviour is unchanged.

## What changed

Every changed line, as `crate: file:line: before -> after`.

**ruma-common 0.20.2** (crates.io; ruma commit `68994653d859bd587de4be5be32861ec63ef5335`)

- `src/room.rs:139`, `:281`, `:598`:
  `let json: Box<RawJsonValue> = Box::deserialize(deserializer)?;`
  -> `let json: Box<RawJsonValue> = <Box<RawJsonValue>>::deserialize(deserializer)?;`

**ruma-events 0.35.2** (crates.io; ruma commit `7d271623c780faeb8e96e49592fb4a220c3ba204`)

- `src/relation/rel_serde.rs:28`:
  `match replace.as_ref().map(Raw::deserialize).transpose() {`
  -> `match replace.as_ref().map(Raw::<Box<E>>::deserialize).transpose() {`

Files added: `LICENSE` in each folder, ruma's MIT licence from the ruma repository at the commit above (the
crates.io packages do not include it). Removed: cargo's `.cargo-ok` marker. `.cargo_vcs_info.json` is kept: it
names the upstream commit.

## Not patched

matrix-sdk 0.19.1 has the same pattern in `src/widget/machine/incoming.rs:114`, but only behind its
`experimental-widgets` feature, which PingCraft does not enable on any target, so it is not vendored. Turning that
feature on needs the same fix there first.

## Upstream

Both ruma sites are still on ruma `main` (2026-11-26: `crates/ruma-common/src/room.rs` lines 143, 285 and 602,
`crates/ruma-events/src/relation/rel_serde.rs` line 28), and matrix-rust-sdk `main` still has the widget one
(`crates/matrix-sdk/src/widget/machine/incoming.rs` line 125). No issue or pull request has been opened yet
(searched 2026-11-26); this section gets the links once one is.

## Removing it

When a ruma release with the fix is out and matrix-sdk depends on it, move matrix-sdk (its own change, ADR 0009),
then delete this folder's crate folders, their `[patch.crates-io]` entries and the `exclude`, and the rows in
`ATTRIBUTION.md`. If craft-i18n stops turning on ICU's `serde` features first, the patch can go then too.
