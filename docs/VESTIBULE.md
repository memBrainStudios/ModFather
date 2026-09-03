# Vestibule

File management member of ModFather. ModFather is the project. FOMOD is the UI. Rhai is the interactive layer inside FOMOD.

## Members

- **7-Zip RE** — immutable source archives; extract into the MGE nested tree; auto-pack to BSA/BA2 when LOOT allows.
- **LOOT** — sort, categories, pack-forbid rules.

Official 7-Zip reference only: [memBrainStudios/7zip](https://github.com/memBrainStudios/7zip) (ip7z/7zip 26.02).

## MOD

A MOD is a state container:

- settings for that MOD's active components
- references to installed archives (hashes, not bytes)

Per-MGE enablement: Off / On / Active. No compressed MOD package.

## MGE

A MGE is a state container:

- settings for active MODs
- their extracted contents
- `loose/` = conflict winners only (last-file-served or player pick)
