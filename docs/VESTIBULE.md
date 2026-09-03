# Vestibule

File management member of ModFather. ModFather is the project. FOMOD is the UI. Rhai is the interactive layer inside FOMOD.

## Members

- **7-Zip RE** — immutable source archives; extract into the MGE nested tree; auto-pack to BSA/BA2 when LOOT allows.
- **LOOT** — sort, categories, pack-forbid rules.

Official 7-Zip reference only: [memBrainStudios/7zip](https://github.com/memBrainStudios/7zip) (ip7z/7zip 26.02).

## MOD

State container: component settings + references to installed archives.

## Enablement vs Active

- **Enabled** — this MGE uses the MOD. Settings come from the MOD.
- **Active** — last MOD the user clicked. Open FOMOD view.
- **Inactive** — every other MOD. Closed row (name or image).

Active is not enablement.
