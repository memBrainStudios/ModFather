# Vestibule

File management member of ModFather.

## Members

- **7-Zip RE** — archive formats, including BSA and BA2; lock/unlock compression of `.mod` packages; read-only navigation of nested archives.
- **LOOT** — sort order for the Vestibule listing and for plugins, including category keys.

UI and FOMOD subscribe to Vestibule. Vestibule subscribes to 7-Zip RE. Nothing else pack/unpacks.

## Reference source

[memBrainStudios/7zip](https://github.com/memBrainStudios/7zip) — fork of [ip7z/7zip](https://github.com/ip7z/7zip) 26.02. Clean-room rewrite only. Do not translate the C/C++.

## `.mod`

- Id: `nexus:{gameDomain}:{modId}`, else `{service}:{id}`, else `hash:{sha256}` of the manifest.
- Holds the family of archives from that source page.
- Nested archives are never altered.
- Locked = compressed, cannot activate. Unlocked = working tree for FOMOD / derived options.
- Slider is binary. Activation is a separate bit.
- First download: LOOT placement, Unlocked, default FOMOD selection.

## FOMOD UI service

Focused Unlocked MOD shows official FOMOD steps plus options derived from every contained archive and from folder layout when no FOMOD exists. Selection is written beside the package; apply copies into staging.
