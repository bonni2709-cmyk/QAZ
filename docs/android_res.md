# android_res — Android launcher resources (Module 6)

(Moved to `docs/`: aapt rejects non-resource entries inside the res tree.)
# android_res — Android launcher resources (Module 6)

Launcher icon (`mipmap-*/ic_launcher.png`), five densities, wired into
`Cargo.toml` via:

```toml
[package.metadata.android]
res = "android_res"
icon = "@mipmap/ic_launcher"
```

and compiled by `cargo quad-apk build` (see the manual `apk` job in
`.github/workflows/build.yml`).

## Provenance — honest placeholder

`tools/gen_icon.py` draws the icon procedurally (geometric primitives only:
railgun coils, armored cab trapezoid, bullbar, amber headlights, hazard chevron
band — no text, no traced artwork). It is a **placeholder** that keeps the APK
honest and installable; it is not studio art. For a commercial release, replace
with master art (keep the same density layout and file names).

Regenerate: `python3 tools/gen_icon.py`
