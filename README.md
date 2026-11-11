# RAILGUN PURSUIT 3D

**Hardcore 3D run-and-gun & survival** — you drive a war rig across a dying
highway: outgun biker gangs, loot caravans, upgrade the truck, and bail out
on foot to scavenge ruins while marauders circle the rig.

- **Engine:** [macroquad](https://macroquad.rs/) 0.4 (3D, immediate mode)
- **Language:** Rust (2021 edition; no `unsafe` anywhere in the crate)
- **Target hardware:** Infinix Note 30 · Helio G99 · Mali-G57 · 8 GB RAM · 60 FPS
- **Shipping budget:** **≥ 200 MB** (4K PBR-ish textures, 5-minute WAV industrial track, high-poly models, multi-ABI APK)

> **Honesty note:** Module 5 generates a **real build-time pack** with
> `python3 tools/gen_assets.py` (deterministic seeds, no network): three 4K
> albedo textures, 14 synthesized SFX and a 5:00 synthesized industrial loop
> — about **57 MB** of actual, game-loaded bytes. They are real files the game
> loads and uses — but they are *procedural placeholders*, not hand-painted 4K
> PBR art or a professionally produced industrial track. The ≥ 200 MB shipping
> budget still stands; the master list that gets us there lives in
> `assets/PLANNED_MASTERS.md` (nothing in it is committed or claimed). The
> loading screen reports real generated sizes — and when the pack is absent it
> says so instead of inventing numbers.

## Module status

| # | Module | Status |
|---|--------|--------|
| 1 | 3D core & transport (truck, procedural road, biomes, chase cam, HUD, loading) | **DONE — this branch** |
| 2 | Combat: 360° turret, overheating, AP/incendiary ammo, bikers/buggies/armored cars, tracers, 3D-part debris | **DONE — this branch** |
| 3 | Looting on the move (magnet beam, wheeled workbench, turret/armor/engine upgrades) | **DONE — this branch** |
| 4 | Weather & lighting (day/night done early, sandstorms), Boss #1: tanker truck | **DONE — this branch** |
| 5 | Heavy assets: 4K textures, WAV audio, 5-min track, multi-arch CI polish | **DONE — this branch (procedural build-time pack)** |
| 6 | UI/menus, JSON saves, APK packaging, icon | **DONE — this branch** |
| 7 | On-foot scavenging: ruin sites, NERVE, marauder skiffs, safes, tool minis (angle grinder / screwdriver) | **DONE — this branch** |
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
- **Loading screen**: 3D truck turntable + asset-stream bar (real build-time
  manifest since Module 5 — see below), hold SPACE/TAP to fast-forward.
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

## Module 4 — what's inside

- **Sandstorm cycle** (`systems/weather.rs`): Clear → Incoming (warning
  banner, dust ramps in ≈6 s) → Storm (13–21 s of gusts, extra tire drag,
  nose-shove gusts, near-whiteout veil + 26 wind streaks) → Clearing → Clear.
  First storm ~40 s into a run, then every 55–95 s. Sky tints to dust, global
  ambient drops ~35 %, HUD gains a pulsing SANDSTORM chip.
- **Boss #1 — JUGGERNAUT** (`entities/boss.rs`): an armored raffinate
  tanker that shows up at ~1300 m and every ~2600 m after.
  - Rubber-band chase (holds ≈24 m ahead; 11 m/s floor so the duel persists);
  - slow sine lane weave + **ram maneuver** every 5–9 s (volatile orange glow
    telegraph under the cab; contact = 26 dmg + shake);
  - **lays proximity mines** off the tail every ~4.2 s (amber → red blink;
    2.35 m trigger, 8 dmg + up to 24 splash);
  - **armor**: AP deals 80 %, incendiary 35 % — bring the tungsten;
  - death: triple explosion, 6× scrap shower + medkit, "JUGGERNAUT DOWN"
    banner, next encounter ≈2600 m later. Top-center integrity bar while it
    lives.
- Day/night cycle (4-minute day) was already in place; storms layer on top of
  it, no art assets required.

## Module 5 — what's inside

- **Build-time asset pack** (`tools/gen_assets.py`, deterministic seeds, no
  network): `textures/asphalt_4k.jpg` (road), `textures/sand_4k.jpg`
  (wasteland/swamp ground, per-biome tinted), `textures/concrete_4k.jpg`
  (ruined-city ground) — plus 14 synthesized SFX (engine loop, storm-wind loop,
  AP/incendiary shots, explosions, ricochet, pickups, overheat alarm, mine
  beep, ram crash, workbench ding) and **`music/industry_legend_5min.wav`** —
  a synthesized 5:00, 100 BPM industrial loop in A minor (~52.9 MB; NOT a
  mastered production). Pack ≈ **57 MB**, git-ignored, produced on every build
  by CI (cached on the generator's hash).
- **Loading screen streams the *real* pack**: `build.rs` renders
  `assets/manifest.json` (real paths + real on-disk bytes) into the binary;
  the bar is driven by the actual bytes the macroquad file IO delivers via a
  loader coroutine — not a simulation. Empty pack? The screen says
  `NO ASSET PACK FOUND — RUN python3 tools/gen_assets.py` instead of lying.
- **Audio is wired into gameplay**: engine loop (volume follows speed), storm
  wind (gain follows sandstorm intensity), music loop on run start, shots,
  ricochets, explosions (small on kills, big on wrecks/boss), overheat alarm,
  pickup chimes, mine beep, ram crash, workbench purchase ding. The wreck
  screen stops engine + music — the song dies with the rig. Restarting
  (`R`) or finishing the load re-arms the loops cleanly (no doubling).
- **World textures**: road plane samples the asphalt albedo; ground slab
  samples sand (wasteland + storm-tinted swamp) or concrete (ruined city).
  Without the pack the renderer falls back to the original vertex colors.
- **CI polish (multi-arch)**: top-level `concurrency` cancel-in-progress; the
  asset pack is generated and integrity-checked (real-byte manifest ≥ 40 MB) on
  **every** job — desktop, all three Android ABIs, and the manual APK job;
  pack cached on the generator's hash. APKs/artifacts now really contain the
  game's audio + textures.
- **Honesty ledger**: the ≥ 200 MB shipping budget still needs real masters —
  painted 4K PBR sets, a mastered industrial track, high-poly models. This
  module proves the *pipeline* end-to-end with procedural placeholders and
  makes every size on screen real; it does not claim artistic production.

## Module 6 — what's inside

- **Main menu** (Loading → **Menu** → Driving): the war rig idles on the
  turntable behind the title; `ENTER` / `SPACE` / tap rolls out (half-second
  input grace so clicks can't skip through). A *service record* strip reads
  straight off the save file: best distance, scrap bank, total runs, total
  kills — the menu is the proof persistence is real.
- **JSON saves** (`systems/savegame.rs`): `railgun_pursuit_save.json` —
  versioned `SaveData` (best distance, scrap, 4 upgrade ranks, career runs/
  kills). Search order: `$RAILGUN_SAVE_PATH` → beside the exe → CWD → temp
  dir. Missing file ⇒ fresh profile; corrupt file ⇒ fresh profile with a
  stderr note (the file is left untouched for manual recovery); write
  failure ⇒ the run continues unsaved. Save flushes: on wreck (career
  counters), on workbench purchase, on run reset / menu abandon, and a 5 s
  debounce when dirty. Upgrades restore across sessions. Android note: the
  sandbox may make every location unwritable — then saves fail soft by
  design (see Honesty note).
- **Pause & WRECKED screens**: `M` abandons the run (saves, back to menu);
  WRECKED now hints `R — ROLL OUT AGAIN · M — MAIN MENU`. Pause footer
  updated to Module 6.
- **APK packaging**: `android_res/mipmap-*/ic_launcher.png` (five densities,
  wired via `res` + `icon = "@mipmap/ic_launcher"` in `Cargo.toml`;
  `version_code` 6, `version_name` `0.1.0-module6`). The launcher icon is a
  **procedural placeholder** (`tools/gen_icon.py` — geometric war-rig front,
  no text) — honest, installable, replaceable by artist masters. The manual
  `apk` CI job builds the APK; the generated asset pack ships inside it.
- **The ≥ 200 MB ledger**: `assets/PLANNED_MASTERS.md` — itemized
  ≈ 430 MB of masters (audio / 4K PBR texture sets / high-poly GLBs) that
  would replace the procedural placeholders. Not committed, not claimed — it
  is the purchase order for a real release.

| 7 | On-foot scavenging: ruin sites, NERVE, marauder skiffs, safes, tool minis (angle grinder / screwdriver) | **DONE — this branch** |

| Desktop | Android |
|---------|---------|
| `W` / `↑` gas · `S` / `↓` brake/reverse | AUTO-GAS throttle · hold left/right screen thirds to steer |
| `A` `D` / `←` `→` steer | — |
| Mouse aims turret · `LMB` / `Space` fire | auto-aim + FIRE button |
| `1` / `2` / `Q` ammo (AP / incendiary / cycle) | tap chips (planned UI) / `Q` equivalent |
| `TAB` wheeled workbench (buy upgrades) | `WB` button |
| `R` reset run | same via pause overlay |
| `Esc` pause · `M` abandon run to menu (paused / wrecked) | Android back button / pause |
| `F` (hold) — bail out at a RUIN SITE | hold on-screen **BAIL** button |
| On foot: `WASD` move · mouse look · `LMB/Space` work · `1`/`2` tools · `E` grab / exfil | auto-walk + screen thirds · **WORK** button holds tools/exfil · tap = grab |
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
- `apk` — manual `workflow_dispatch` job, produces a debug-signed APK via `cargo-quad-apk`
  (icon + five-density mipmaps wired, asset pack generated and packaged inside).
  **Not yet dispatched**: the CI integration used for this branch cannot trigger
  `workflow_dispatch` (HTTP 403), so the APK itself has not been produced yet.
  Everything up to the APK boundary is CI-green (all three Android ABIs
  cross-compile with the audio backend + icon resources). Run the job once from
  the Actions tab to mint the first APK artifact.

## Project layout

```
src/
  main.rs               — entry point, state machine bootstrap
  states/               — app state machine (Loading → Menu → Driving ⇄ OnFoot)
    loading.rs          — build-time asset-stream loading screen
    menu.rs             — main menu + service record (reads the JSON save)
    driving.rs          — the endless-highway run (+ ruin-site bail logic)
    onfoot.rs           — on-foot salvage episode (Module 7): seeded ruin bowl
  entities/
    truck.rs            — the war rig (build, physics, draw)
    projectiles.rs      — pooled AP/incendiary/rockets (zero alloc)
    enemies.rs          — biker / buggy / armored car + pooled AI
    boss.rs             — JUGGERNAUT raffinate tanker + pooled proximity mines
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
    weather.rs          — sandstorm cycle (roll, gusts, banners)
    assets.rs           — runtime bundle: manifest, loader coroutine, playback
    savegame.rs         — JSON save game (versioned SaveData, atomic-ish paths)
  ui/
    hud.rs              — bars, speedometer, banners, letterbox/vignette
    workbench.rs        — wheeled-workbench upgrade panel (TAB/WB)
build.rs                — renders assets/manifest.json → loading-screen table
tools/gen_assets.py     — deterministic build-time pack generator (git-ignored output)
assets/                 — generated at build (git-ignored): textures/ audio/ music/
  PLANNED_MASTERS.md    — the ≥200 MB master-asset purchase order (not in repo)
android_res/            — launcher icons, five densities (procedural placeholder)
.github/workflows/      — desktop + Android ABI CI, manual APK job, asset-pack cache
```

## License

MIT — see `LICENSE`.
