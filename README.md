# ModFather

**Native PC application suite**, not a web app. Clean-room **Rust** toolchain and desktop shell for the Bethesda Game Studios modding workflow, with **Rhai** as the only first-class scripting surface.

ModFather is an integrator, not an omnibus, and it replaces the overlapping legacy tool stack in one umbrella product: Mod Organizer-shaped environment + virtual file system, xEdit-class plugin record editing, LOOT-class placement/sort, NifSkope-class mesh editing, and per-game script-extender launch. Reference forks on the account are a capability catalog and format-knowledge source — clean-room only, never wrapped or line-translated.

## System map

One-way custody chain:

```
7-Zip RE  →  Vestibule  →  Crucible  →  ModFather (shell / integrator)
```

- **7-Zip RE** — clean-room, full standalone Rust implementation of the 7z container and its codecs, ships as its own redistributable package. BSA (v103–105) and BA2 (GNRL/DX10) are **separate Bethesda archive extension crates** that plug into 7-Zip RE's container registry — not bundled into the standalone package. The registry is a real GoF Strategy + Factory mechanism, not just prose: `sevenzip_re::container` defines a `ContainerFormat`/`ContainerHandle` trait pair and a `Registry` that probes magic bytes and dispatches; each format crate implements the pair for its own archive type (`sevenzip_re::container::SevenZipFormat`, `modfather_bsa::container::BsaFormat`, `modfather_ba2::container::Ba2Format`), and `modfather-vestibule::container::build_registry` is where all three are assembled into one shared `Registry` (the first crate in the custody chain that already depends on every format). RAR is a placeholder pending license, in neither deliverable until then; once licensed, it registers as a fourth `ContainerFormat` with no changes needed anywhere else. Reference tree (26.02): https://github.com/memBrainStudios/7zip — rewrite, do not translate.
- **Vestibule** — file management only: VFS roots (game install, save/config, Data), last-wins layering with explicit 1:1 picks, compare tooltips, and **LOOT** (generalized sorter: Nexus categories, traditional masterlist, user rules). Details: [docs/VESTIBULE.md](docs/VESTIBULE.md).
- **Crucible** — the editor system-of-systems: Records (xEdit-class), NIF (NifSkope-class, with BodySlide/Outfit Studio as features), Materials, Textures, Papyrus, Animation/LOD, Localization/Audio/UI/Saves. Depends on Vestibule only. Details: [docs/CRUCIBLE.md](docs/CRUCIBLE.md).
- **ModFather shell** — native desktop UI and per-game extender launch. FOMOD is the UI; Rhai is the interactive layer inside it. Never a web/Electron stack.

**MOD** is a state container keyed by Nexus id (or another store id, or a content hash): settings for its components plus references to installed archives — never embedded archive bytes. **MGE** (ModFather Game Environment) is a state container for lock-slider inclusion/order, last-click Active, the extracted VCS tree of included MODs, and the conflict overlay.

Full wave-by-wave build order: [docs/SCHEDULE.md](docs/SCHEDULE.md).

The misnamed `anvil` repository was a first-pass scaffold. New work goes here.

Suggested crate prefix (change if you want a different one): `modfather-*`. The standalone 7z engine is `sevenzip-re`; BSA/BA2 are separate extension crates under the `modfather-*` prefix, not folded into `sevenzip-re`.

## Principles

- Traits in the center, editions at the edge (SOLID).
- Lazy mmap indexes; typed views on demand.
- Scripts receive an `AssetHost` with grants. No raw filesystem in the sandbox.
- Mutations are `Op` values. Apply is a separate, journaled step.
- Nested archives are never altered.
- No in-process game hooking in this repository.
- Native desktop process throughout — no web/browser/Electron stack in the product.

## License

MIT OR Apache-2.0 for ModFather sources. Contains no Bethesda assets and no copied third-party tool source. The 7zip reference repo keeps its upstream license.
