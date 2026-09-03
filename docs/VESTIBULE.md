# Vestibule

File management member of ModFather. ModFather is the project. FOMOD is the UI. Rhai is the interactive layer inside FOMOD.

## Members

- **7-Zip RE** — source archives (7z, zip, BSA, BA2, …) are immutable. Extract into the MGE tree. Auto-pack loose files to BSA/BA2 when LOOT allows.
- **LOOT** — sort, categories, and pack-forbid rules.

Official 7-Zip reference only: [memBrainStudios/7zip](https://github.com/memBrainStudios/7zip) (ip7z/7zip 26.02). No zstd fork.

## MOD

A MOD **references** archives. It does not embed them. It stores per-MGE state: which files are present, FOMOD selection, and Off / On / Active.

- Off — not contributing. Closed row.
- On — contributing. Closed row (full name or image).
- Active — selected in FOMOD. Open view. Requires On. One Active at a time.

No compressed MOD package. No lock slider.

## Derived FOMOD

- Radio buttons when destination paths conflict.
- Checkboxes when they do not.

FOMOD state is the access key into that MOD’s nested folders.

Compatibility steps that name other MODs: present MODs are available; missing MODs are requested from the host (collection-style pull). Conflicts among On MODs are bashed.
