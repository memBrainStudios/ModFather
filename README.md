# ModFather

Clean-room **Rust** toolchain for Bethesda Game Studios plugin, archive, and asset workflows, with **Rhai** as the only first-class scripting surface.

This is not a wrapper around xEdit, Mutagen, MO2, Vortex, Wrye Bash, or the script extenders. Forks on this account are reference archives. ModFather re-derives the formats and keeps one domain model.

The misnamed `anvil` repository was a first-pass scaffold created before this name was given. New work goes here.

Suggested crate prefix (change if you want a different one): `modfather-*`.

| Suggested crate | Role |
|---|---|
| `modfather-core` | Form IDs, game IDs, errors, session traits |
| `modfather-codec` | Endian, compression, FourCC, framed I/O |
| `modfather-plugin` | ESM / ESP / ESL index |
| `modfather-archive` | BSA / BA2 |
| `modfather-rhai` | Sandboxed script host |
| `modfather-cli` | `inspect` / `extract` / `script` |

Game editions implement `EditionCodec`. They do not fork the domain types.

## Principles

- Traits in the center, editions at the edge (SOLID).
- Lazy mmap indexes; typed views on demand.
- Scripts receive an `AssetHost` with grants. No raw filesystem in the sandbox.
- Mutations are `Op` values. Apply is a separate, journaled step.
- No in-process game hooking in this repository.

## Status

Workspace created. Phase 1: parse a TES5/SSE plugin header and GRUP tree, list a BSA.

## License

MIT OR Apache-2.0. Contains no Bethesda assets and no copied third-party tool source.
