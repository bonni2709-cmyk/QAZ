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
| 2 | Combat: 360° turret, overheating, AP/incendiary ammo, bikers/buggies/armored cars, tracers, 3D-part debris | **DONE — this branch** |
| 3 | Looting on the move (magnet beam, wheeled workbench, turret/armor/engine upgrades) | **DONE — this branch** |
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

## Module 2 — what's inside

- **360° manual turret** (mouse-look on desktop, auto-aim with target lead on
  Android): hold LMB / Space to fire, ~8.7 rds/s, 360° free traverse.
- **Heat model**: sustained fire walks the barrel into an overheat lockout
  (unlocks at 35 %); incendiary runs cooler than AP.
- **Two ammo types** (`1` / `2` / `Q`): Tungsten AP (flat 34 dmg, ignores
  armor, 30 % ricochet off armored cars) and Incendiary (24 dmg vs
  unarmored / 14 vs armored, +9 dps burn for 3 s).
- **Three enemy machines**, each with its own low-poly model and AI:
  - **Raider Biker** — 27.5 m/s, weaves on a sine through traffic, SMG on a
    cooldown with range-falloff hit probability, rams at close range;
  - **Junker Buggy** — lateral jerks + periodic ballistic jump, rams and
    damages itself doing it;
  - **Armored Car** — holds ~50 m stand-off, lazy sidestep, fires unguided
    rockets with partial target lead on a telegraphed cooldown (muzzle glow
    0.65 s before launch); armored — AP's prey.
- **Wave director** — difficulty scales with distance (0.9 → 2.8), concurrency
  cap grows 2 → 7, spawn mix shifts from bikers to armored cars past 250 m,
  6 s spawn grace at run start, occasional biker rear-ambushes.
- **Effects** — pooled particles: explosion fireballs, smoke rings, spark
  bursts, bouncing 3D debris, muzzle flashes, rocket smoke breadcrumbs,
  burn-DoT flames; hitscan tracers for the SMG; flat bullet tracers (AP
  yellow / incendiary orange).
- **Hull damage & wreck**: enemy SMG / rams / rockets chew the HULL bar;
  at 0 the rig is **WRECKED** — big explosion, run summary (distance/best/
  kills), `R` rolls out again. Nearby rockets shake the camera.
- **HUD**: ammo chips, KILLS counter, hit / kill markers, INCOMING warning
  when a rocket is inbound, OVERHEATED flash, FIRE button on Android.

## Module 3 — what's inside

- **Salvage drops on kill**: enemy wrecks spill scrap crates with a toss
  (biker 6–10, buggy 5–10 ×2, armored car 8–13 ×3) — +25 % buggy chance to
  drop fuel, +40 % armored-car chance to drop a medkit.
- **Crate clusters seeded by the wave director** every 8–13 s ahead of the rig
  (≈78 % on the asphalt, rest on the shoulder): scrap / fuel cans / medkits.
- **Magnet beam** (always on): anything within beam radius is reeled into the
  rig at up to 24 m/s and consumed on contact (+18 HULL per medkit, +22 FUEL
  per can); magnetised pickups draw a hot filament from the truck to themselves.
- **Wheeled Workbench** (`TAB`, Android `WB` button; world pauses while
  open): the bed-mounted upgrade bench with four tracks, 3 ranks each:
  - **COMPOSITE PLATING** — +25 max HULL and 6 % flat damage resist per rank
    (buying ranks mid-run patches the rig by the delta);
  - **AUTOLOADER & COOLING** — +9 % fire rate, +18 % cooling per rank;
  - **BIG-BLOCK ENGINE** — +10 % top speed, +12 % acceleration per rank;
  - **SALVAGE MAGNET** — +2.4 m beam radius per rank (6 m base → 13.2 m max).
- **Economy**: scrap persists for the session (runs you wreck out of still
  keep it); JSON saves arrive in Module 6. HUD tracks the bank next to KILLS;
  wreck screen shows distance/best/kills/scrap bank.
- **UI**: workbench panel with rank pips, FIT buttons, insufficient-funds
  dimming, purchase flash; desktop + Android hit-testing (touches simulate
  mouse).

## Controls

| Desktop | Android |
|---------|---------|
| `W` / `↑` gas · `S` / `↓` brake/reverse | AUTO-GAS throttle · hold left/right screen thirds to steer |
| `A` `D` / `←` `→` steer | — |
| Mouse aims turret · `LMB` / `Space` fire | auto-aim + FIRE button |
| `1` / `2` / `Q` ammo (AP / incendiary / cycle) | tap chips (planned UI) / `Q` equivalent |
| `TAB` wheeled workbench (buy upgrades) | `WB` button |
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
    projectiles.rs      — pooled AP/incendiary/rockets (zero alloc)
    enemies.rs          — biker / buggy / armored car + pooled AI
  world/
    road.rs             — procedural chunks, biomes, deterministic PRNG
  systems/
    meshkit.rs          — rotated-part mesh kit (macroquad primitives are axis-aligned!)
    chase.rs            — chase camera (+ impact shake)
    particles.rs        — pooled fire/smoke/sparks/debris/tracers
    turret.rs           — 360° turret controller, heat model, ammo
    director.rs         — wave director (difficulty curve + crate seeding)
    loot.rs             — pooled salvage pickups, magnet beam pull, beacons
    upgrades.rs         — scrap economy, 4 workbench tracks (3 ranks each)
  ui/
    hud.rs              — bars, speedometer, banners, letterbox/vignette
    workbench.rs        — wheeled-workbench upgrade panel (TAB/WB)
assets/                 — textures/ audio/ models/ music/ (Module 5)
.github/workflows/      — desktop + Android ABI CI, manual APK job
```

## License

MIT — see `LICENSE`.
