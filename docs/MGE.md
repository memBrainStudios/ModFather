# MGE — ModFather Game Environment

An MGE is a state container.

It holds:

- settings for the active MODs (Off / On / Active, order)
- extracted contents of those MODs (nested `mods/<MOD>/<archive>/`)
- loose files **only** when they resolve a conflict

Conflict overlay (`loose/`):

- **Last-file-served** — later On MOD wins the path, or
- **Player option** — the player chooses the winner

Non-conflicting extracts stay under the MOD folder. They are not copied into `loose/`.

New MGE: every viable downloaded MOD starts Off. Switch MGE without restart. Version-controlled. Themeable.
