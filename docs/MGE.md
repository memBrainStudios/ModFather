# MGE — ModFather Game Environment

An MGE is a state container for:

- which MODs are enabled, and in what order
- last click (which MOD is Active)
- extracted contents of enabled MODs
- `loose/` conflict winners only (last-file-served or player pick)

It does **not** own component settings. It reads those from the enabled MODs.

Active = last click. Inactive = all other MODs.
