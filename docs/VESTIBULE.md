# Vestibule

File management member of ModFather. ModFather is the project. FOMOD is the UI. Rhai is the interactive layer inside FOMOD.

## Members

- **7-Zip RE** — immutable source archives; extract download-repo → VCS; pack packable loose per Bethesda doctrine when LOOT allows.
- **LOOT** — sort, categories, pack-forbid rules.

Official 7-Zip reference only: [memBrainStudios/7zip](https://github.com/memBrainStudios/7zip) (ip7z/7zip 26.02).

## Enable a feature / compatibility

External MOD **not present** → pull request generated and resolved.
External MOD **present and inactive** → request to automatically activate.

A **collection** batches that walk. Multiple collections may run in one session.

## Pull

1. Write files to `download-repo/<MOD name>/`. Do not extract there.
2. Extract each archive to `VCS/mods/<MOD name>/<archive name>/`.
3. Pack packable loose:
   - BA2: `{stem} - Main.ba2`, `{stem} - Textures.ba2`
   - BSA: `{stem}.bsa`, `{stem} - Textures.bsa` — never ` - Main` on BSA
4. Mandatory loose stays in `VCS/mods/<MOD name>/loose/`.

Extract is pull-driven. Click sets Active (if unlocked) and opens FOMOD so the user changes which assets in that MOD hierarchy are used. Click does not extract.

## Bash

Plugins and loose assets. If the player picked winners, those criteria win. Otherwise automatic bash.

## MOD

State container: component settings + references to installed archives.

## Enablement vs Active

- **Enabled** — this MGE uses the MOD. Settings come from the MOD.
- **Active** — last click, if unlocked. Open FOMOD view.
- **Inactive** — every other MOD.
