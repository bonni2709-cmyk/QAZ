# RAILGUN PURSUIT 3D

**Hardcore 3D run-and-gun & survival** — you drive a war rig across a dying
highway: outgun biker gangs, loot caravans, upgrade the truck, and (from
Module 7) bail out on foot to scavenge ruins while marauders circle the rig.

- **Engine:** [macroquad](https://macroquad.rs/) 0.4 (3D, immediate mode)
- **Language:** Rust (2021 edition; no `unsafe` anywhere in the crate)
- **Target hardware:** Infinix Note 30 · Helio G99 · Mali-G57 · 8 GB RAM · 60 FPS
- **Shipping budget:** **≥ 200 MB** (4K PBR-ish textures, 5-minute WAV industrial track, high-poly models, multi-ABI APK)

> **Honesty note:** Modules 5+ bring the real 4K assets. Until then the loading
> screen streams a *manifest* whose sizes already sum to ~268 MB — the number
> on screen is the shipping budget, not recycled dummy data. No fake "200 MB"
> padding is committed to this repository.

## Module status

| # | Module | Status |
|---|--------|--------|
| 1 | 3D core & transport (truck, procedural road, biomes, chase cam, HUD, loading) | **DONE — this branch** |
| 2 | Combat: 360° turret, overheating, AP/incendiary ammo, bikers/buggies/armored cars, tracers, 3D-part debris | next |
| 3 | Looting on the move (magnet beam, wheeled workbench, turret/armor/engine upgrades) | planned |
| 4 | Weather & lighting (day/night done early, sandstorms), Boss #1: tanker truck | planned |
| 5 | Heavy assets: 4K textures, WAV audio, 5-min track, multi-arch CI polish | planned |
| 6 | UI/menus, JSON saves, APK packaging, icon | planned |
| 7 | On-foot scavenging: outposts, safes, tool minis (angle grinder / screwdriver) | planned |
| 8 | Factions, global bosses (crawler worm, bomber drones), New Game+, Codex | planned |

## Module 1 — what's inside

- **Detailed low-poly war rig** (~30 primitives, ~1.3 k verts, 3 draw calls):
  chassis, armored cargo bed with bolt-on plates, armored cab, bullbar,
  dual exhaust stacks, roof hatch, antenna, glass, mud flaps, fuel tanks,
  emissive head/tail lights, scanning roof turret, six spinning wheels
  (front axle + dual rear axles).
- **Procedural endless highway** along +Z: asphalt, edge lines, dashed
  centerline, soft-sand shoulders that bleed speed, and a seamless biome
  cycle — *Wasteland → Toxic Swamp → Ruined City* — each with deterministic
  seeded props (cacti, boulders, wrecked cars / sludge pools, dead trees,
  toxic barrels / collapsed towers, rubble, burnt buses).
- **Day/night cycle** (4-minute full day), baked per-vertex sun shading with
  a global ambient factor, biome-tinted dusk/dawn/night skies, headlight
  emissives for night driving.
- **Chase camera**: smoothed follow, speed-reactive FOV (55°→68°),
  steer counter-lean, subtle breathing sway + 2D letterbox/vignette
  ("tilt-shift" flavour without render-target cost).
- **HUD** (macroquad immediate mode): HULL / FUEL / TURRET TEMP bars, big
  speedometer, distance + session best, biome chip, biome-transition banner,
  FPS readout, Android touch zones with AUTO-GAS.
- **Loading screen**: 3D truck turntable + asset-stream bar (~268 MB manifest),
  hold SPACE/TAP to fast-forward.
- **Code health**: no `unwrap()` anywhere, zero per-frame heap traffic for
  rendering (parts re-bake into reused vertex buffers), deterministic world
  (stable across sessions), unit tests for the PRNG/biome bands.

## Controls

| Desktop | Android |
|---------|---------|
| `W` / `↑` gas · `S` / `↓` brake/reverse | AUTO-GAS throttle · hold left/right screen thirds to steer |
| `A` `D` / `←` `→` steer | — |
| `R` reset run | same via pause overlay |
| `Esc` pause | Android back button / pause |
| hold `Space` / click on loading = fast-forward | tap & hold anywhere |

## Build & run

```bash
# desktop
cargo run --release

# tests & lints
cargo test
cargo clippy --all-targets

# Android (Infinix Note 30) — see macroquad docs (cargo-quad-apk)
cargo install cargo-quad-apk
cargo quad-apk build --release        # APK → target/android-artifacts/release/apk/
```

CI (`.github/workflows/build.yml`):

- `desktop-check` — fmt (advisory), clippy (advisory), `cargo check`,
  `cargo test`, `cargo build --release`, uploads a Linux binary;
- `android` — release cross-compile of all three ABIs the Note 30 may need
  (`aarch64-linux-android` primary, `armv7-linux-androideabi`, `x86_64-linux-android`)
  using the NDK preinstalled on GitHub runners;
- `apk` — manual `workflow_dispatch` job, produces a debug-signed APK via `cargo-quad-apk`.

## Project layout

```
src/
  main.rs               — entry point, state machine bootstrap
  states/               — app state machine (Loading → Driving)  [menu/garage in Module 6]
    loading.rs          — 200+ MB asset-stream loading screen
    driving.rs          — the endless-highway run
  entities/
    truck.rs            — the war rig (build, physics, draw)
  world/
    road.rs             — procedural chunks, biomes, deterministic PRNG
  systems/
    meshkit.rs          — rotated-part mesh kit (macroquad primitives are axis-aligned!)
    chase.rs            — chase camera
  ui/
    hud.rs              — bars, speedometer, banners, letterbox/vignette
assets/                 — textures/ audio/ models/ music/ (Module 5)
.github/workflows/      — desktop + Android ABI CI, manual APK job
```

## License

MIT — see `LICENSE`.
