# MGE — ModFather Game Environment

State container for enabled MODs, last click (Active), and the VCS extract tree.

```
download-repo/<MOD>/          # unextracted pulls
VCS/mods/<MOD>/<archive>/     # extract
VCS/mods/<MOD>/loose/         # mandatory loose only
```

Component settings live on the MOD. The MGE reads enabled MODs.

Cross-MOD conflicts: last-file-served or player pick. That overlay does not replace per-MOD `loose/`.
