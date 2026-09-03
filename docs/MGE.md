# MGE — ModFather Game Environment

The MGE virtual file system is a version-controlled install tree.

```
<MGE>/
  mods/
    <MOD name>/          # one root per MOD
      <archive>/         # one folder per referenced archive (extracted once)
      fomod-state        # access key
  loose/                 # the only loose-file collection
```

- References MODs. Source zips stay in the archive store.
- Per-MGE state: Off / On / Active, downloads present, FOMOD selection.
- New MGE: every viable downloaded MOD starts **Off**.
- Switch MGE without restart. Vestibule remounts live.
- Version-controlled. That version pins MOD identities, archive hashes, and selections.
- Themeable. Save an MGE as a theme (collection analogue).
- After FOMOD selection, loose files that belong in BSA/BA2 are packed by 7-Zip RE unless a LOOT rule forbids it. Remainder goes to `loose/`.
- Never a second loose collection.
