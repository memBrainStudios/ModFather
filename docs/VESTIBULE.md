# Vestibule

File management member of ModFather. ModFather is the project. FOMOD is the UI. Rhai is the interactive layer inside FOMOD.

## Members

- **7-Zip RE** — immutable source archives; extract download-repo → VCS; pack packable loose per Bethesda doctrine when LOOT allows. RAR extraction is a placeholder pending license.
- **LOOT** — generalized sorter: Nexus categories (user may use or modify them), traditional masterlist, and user-created rules.

Official 7-Zip reference only: [memBrainStudios/7zip](https://github.com/memBrainStudios/7zip) (ip7z/7zip 26.02).

## 7-Zip RE is two deliverables

- **Standalone package** (`sevenzip-re`) — a complete, independent Rust implementation of the 7z container and its core codecs (Copy, LZMA, LZMA2; more filters later). No Bethesda-specific code, no game dependency. It is redistributable on its own. RAR stays a placeholder pending license and ships in neither deliverable until then.
- **Bethesda archive extension** (separate crate(s), e.g. `modfather-bsa`, `modfather-ba2`) — BSA (v103–105) and BA2 (GNRL/DX10) are Bethesda's own container formats, not 7z. They plug into 7-Zip RE's container registry as additional handlers, equal in standing to 7z, but they are **not bundled into the standalone package**. Vestibule depends on the standalone package plus these extension crates; a consumer who only wants general-purpose 7z support does not have to pull in Bethesda format code.

### The container registry, concretely

"Plug into 7-Zip RE's container registry as additional handlers" is a real mechanism, not just this paragraph's prose: `sevenzip-re` ships a GoF Strategy + Factory pair in `sevenzip_re::container` --

- `ContainerFormat` / `ContainerHandle` — the Strategy interface every format implements once (probe magic bytes, open, list entries, read an entry by index).
- `Registry` — the Factory: holds every registered `ContainerFormat`, peeks a stream's header bytes, and dispatches `Registry::open` to whichever format's `probe` matches.

`sevenzip-re` ships this mechanism **empty** plus its own `SevenZipFormat`/`SevenZipHandle` payload for 7z; it never registers BSA/BA2 itself, since that would mean the standalone package depending on Bethesda-specific crates, inverting the one-way custody chain. Each extension crate instead implements the same trait pair for its own archive type (`modfather_bsa::container::BsaFormat`, `modfather_ba2::container::Ba2Format`), and `modfather-vestibule::container::build_registry` is where all three are assembled into the one shared `Registry` actually used by Vestibule and anything downstream of it -- Vestibule is the first crate in the chain that already depends on every format extension, so it is the only correct place to do this without inverting that dependency direction. A future RAR crate (once licensed) slots in the same way: implement the trait pair, add one line to `build_registry`, and every existing caller of `Registry::open` picks it up with no other code changes.

## Lock slider

Two-position control on the MGE. Unlocked = this MOD is explicitly included in the MGE. Locked = not included. Not compression. New MGE: viable MODs start Locked.

MOD settings decide what content that included MOD contributes.

## Compatibility

External MOD **not present** → pull request generated and resolved.
External MOD **present and inactive** → request to automatically activate (slider Unlocked, then Active).

A **collection** batches that walk. Multiple collections may run in one session.

## Pull

1. Write files to `download-repo/<MOD name>/`. Do not extract there.
2. Extract each archive to `VCS/mods/<MOD name>/<archive name>/`.
3. Pack packable loose:
   - BA2: `{stem} - Main.ba2`, `{stem} - Textures.ba2`
   - BSA: `{stem}.bsa`, `{stem} - Textures.bsa` — never ` - Main` on BSA
4. Mandatory loose stays in `VCS/mods/<MOD name>/loose/`.

Extract is pull-driven. Click sets Active if Unlocked and opens FOMOD so the user changes which assets in that MOD hierarchy are used. Click does not extract.

## Bash

Plugins and loose assets. If the player picked winners, those criteria win. Otherwise automatic bash among Unlocked MODs.
