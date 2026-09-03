# Vestibule

File management member of ModFather. ModFather is the project. FOMOD is the UI. Rhai is the interactive layer inside FOMOD.

## Members

- **7-Zip RE** — immutable source archives; extract download-repo → VCS; pack packable loose per Bethesda doctrine when LOOT allows. RAR extraction is a placeholder pending license.
- **LOOT** — generalized sorter: Nexus categories (user may use or modify them), traditional masterlist, and user-created rules.

Official 7-Zip reference only: [memBrainStudios/7zip](https://github.com/memBrainStudios/7zip) (ip7z/7zip 26.02).

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
