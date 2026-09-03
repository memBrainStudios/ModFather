# ModFather

Clean-room **Rust** toolchain for Bethesda Game Studios plugin, archive, and asset workflows, with **Rhai** as the only first-class scripting surface.

File management lives in **Vestibule**. Vestibule members: **7-Zip RE** (all archives, including BSA/BA2, and `.mod` lock/unlock) and **LOOT** (sort, including categories). Mods are `.mod` packages keyed by Nexus id (or another store id, or a content hash).

Reference 7-Zip tree (26.02): https://github.com/memBrainStudios/7zip — rewrite, do not translate.

Details: [docs/VESTIBULE.md](docs/VESTIBULE.md).

The misnamed `anvil` repository was a first-pass scaffold. New work goes here.

Suggested crate prefix (change if you want a different one): `modfather-*`. Archive work is suggested as `sevenzip-re`, not a separate BSA crate.

## Principles

- Traits in the center, editions at the edge (SOLID).
- Lazy mmap indexes; typed views on demand.
- Scripts receive an `AssetHost` with grants. No raw filesystem in the sandbox.
- Mutations are `Op` values. Apply is a separate, journaled step.
- Nested archives are never altered.
- No in-process game hooking in this repository.

## License

MIT OR Apache-2.0 for ModFather sources. Contains no Bethesda assets and no copied third-party tool source. The 7zip reference repo keeps its upstream license.
