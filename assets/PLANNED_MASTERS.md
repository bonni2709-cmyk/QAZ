# PLANNED MASTERS — the path to a real ≥ 200 MB shipping pack

**Status: NOT IN THIS REPOSITORY.** This file is the manifest of real,
licensed/commissioned master assets that would replace the Module 5 procedural
placeholders and bring the shipping pack to its ≥ 200 MB budget. Nothing here
is faked, committed, or claimed as present. `tools/gen_assets.py` stays the
build-time generator until each line below is delivered and wired.

## Audio (WAV/FLAC masters, 44.1 kHz)

| Path | Content | Est. size |
|---|---|---|
| `music/main_theme_industrial.wav` | Main industrial track, 5:00, mastered (stereo, true peaks −1 dBTP) | 53 MB |
| `music/combat_layer_stems/*.wav` | 4 stems (perc / bass / metal FX / siren lead) for dynamic mix | 95 MB |
| `audio/*.wav` | 40+ SFX masters: engines, gunfire, impacts, UI, ambience (replaces 14 synth slots 1:1) | 60 MB |
| `audio/voice_radio.wav` | Radio chatter / announcer VO (nothing synthesized yet) | 6 MB |

## Textures (4K PBR sets: albedo + normal + roughness + AO, JPEG/PNG)

| Path | Content | Est. size |
|---|---|---|
| `textures/asphalt_*` | Road set, 4 maps | 22 MB |
| `textures/sand_*` | Dune set, 4 maps | 18 MB |
| `textures/concrete_*` | Ruined-city set, 4 maps | 20 MB |
| `textures/metal_hull_*` | Truck hull + bolts, 4 maps | 26 MB |
| `textures/sky_8k.hdr` | HDR environment for the 3D pass | 38 MB |
| `textures/codex_*` | Codex spreads for Module 8 | 7 MB |
| `textures/ui_kit_*` | Menu/garage/HUD kit, 4K | 12 MB |

## Models (high-poly + LODs, .glb)

| Path | Content | Est. size |
|---|---|---|
| `models/truck_highpoly.glb` | War rig, interior-less | 13 MB |
| `models/tanker_boss.glb` | JUGGERNAUT tanker + mines | 12 MB |
| `models/armoredcar.glb`, `models/buggy.glb`, `models/biker.glb` | Enemy machines | 29 MB |
| `models/crawler_worm.glb`, `models/bomber_drone.glb` | Module 8 bosses | 11 MB |
| `models/outpost_ruins.glb`, `models/merchant_convoy.glb` | Module 7 scenery | 7 MB |

**Rough total: ≈ 430 MB of masters** — well over the ≥ 200 MB budget; the
final pack size is set at Module 8 polish, pruned by what actually ships.
