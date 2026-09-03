# Vestibule

File management member of ModFather. ModFather is the project. FOMOD is the UI. Rhai is the interactive layer inside FOMOD.

## Members

- **7-Zip RE** — archive formats, including BSA and BA2; lock/unlock compression of `.mod` packages; read-only navigation of nested archives.
- **LOOT** — sort order for the Vestibule listing and for plugins, including category keys.

FOMOD subscribes to Vestibule. Vestibule subscribes to 7-Zip RE. Vestibule mounts the current **MGE**.

## `.mod`

- Id: `nexus:{gameDomain}:{modId}`, else `{service}:{id}`, else `hash:{sha256}` of the manifest.
- Holds the family of archives from that source page.
- Nested archives are never altered.
- Locked = compressed. Cannot become Active until Unlocked.
- **Active** = the user selected this MOD in FOMOD. Open view. Inactive = one line (full name) or a representative image.

## Derived FOMOD

- Radio buttons when destination paths conflict.
- Checkboxes when they do not.

## MGE

See [MGE.md](MGE.md).
