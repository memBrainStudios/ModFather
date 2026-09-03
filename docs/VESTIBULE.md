# Vestibule

File management member of ModFather. ModFather is the project. FOMOD is the UI. Rhai is the interactive layer inside FOMOD.

## Members

- **7-Zip RE** — immutable source archives; extract download-repo → VCS; pack packable loose to plugin-stem BSA/BA2 when LOOT allows.
- **LOOT** — sort, categories, pack-forbid rules.

Official 7-Zip reference only: [memBrainStudios/7zip](https://github.com/memBrainStudios/7zip) (ip7z/7zip 26.02).

## Enable a feature

If a dependency is in the store, request enable. If it is not, pull from the host.

A **collection** is that walk, batched. Multiple collections may run in one session. Same download repo, VCS, enablement, and conflict overlay.

## Pull

1. Write files to `download-repo/<MOD name>/`. Do not extract there.
2. Extract each archive to `VCS/mods/<MOD name>/<archive name>/`.
3. Pack packable loose as `{PluginStem} - Textures` (textures) or `{PluginStem} - Main` (everything else). `.ba2` or `.bsa` by edition. BSA uses that game's auto-load pair.
4. Mandatory loose stays in `VCS/mods/<MOD name>/loose/`.

Extract is pull-driven. Click and enable-of-store-resident do not extract.

## Bash

Plugins and loose assets. If the player picked winners, those criteria win. Otherwise automatic bash.

## MOD

State container: component settings + references to installed archives.

## Enablement vs Active

- **Enabled** — this MGE uses the MOD. Settings come from the MOD.
- **Active** — last click. Open FOMOD view.
- **Inactive** — every other MOD.
