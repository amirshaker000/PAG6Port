# Scarlett fork (branch `scarlett-fox`), state of 2026-11-26

## How to start the game (the short version)

You need your own dump of the game: a folder that has `game_scarlett_master.exe` in it. **This repository contains
no game files, code or assets.**

1. **Install the tools once:** [Rust](https://rustup.rs) and the Visual Studio C++ build tools with a Windows SDK.
2. **Build once.** Open PowerShell in this folder and run:

   ```powershell
   cargo build --profile diag -p victory --features diagnostics --locked
   ```

3. **Start the game.** Drag your game folder onto **`Play-Scarlett.cmd`**.
   After the first time, just double-click `Play-Scarlett.cmd`: it remembers the folder.

That is all. `Play-Scarlett.cmd` sets every setting and memory hook this branch is measured with, in Series X mode.

Good to know:

- **The first start is slow** and can show missing faces: the game's shaders are translated once and kept. From the
  second start on it is right.
- **Controls:** an Xbox controller works. On the keyboard: W A S D move, I J K L camera, Enter = A, Backspace = B,
  X and Y, Z / C = left / right trigger, Q / E = LB / RB, arrows = D-pad, Esc = Menu, Tab = View.
- **Do not use `Play-VIctory.cmd`** for this: that is the base's launcher, with Series S and other settings.
- **Ray tracing** is off by default (the game's own, much slower). To turn it on, start it from PowerShell:

  ```powershell
  .\scripts\play-scarlett.ps1 -RayTracing
  ```

- **Something looks wrong?** Try without the memory hooks: `.\scripts\play-scarlett.ps1 -NoHooks`
- More options are listed at the top of `scripts\play-scarlett.ps1` (other cache or save folders, a release build,
  automatic start-screen presses, any single setting through `-More`).

## What this is

This branch is codec-d/cool_fox at `f4b48ad` (itself avatarmanz/VIctory-7900XTX `0a43566` plus six commits) with the
work of a local fork ("Scarlett") ported onto it. Target PC: Ryzen 7 9800X3D, 32 GB RAM, RTX 4080 SUPER 16 GB,
2560x1440, Series X mode. Goal: a stable 30 frames a second with no loss of picture. It is not there yet.

## Measured (6-minute opening route, diagnostics build, ray tracing off)

| | cool_fox as it was (floor) | this branch, current settings |
|---|---|---|
| ms per frame, 1,000-2,500 draws | 50-55 | 35 |
| ms per frame, 2,500-4,000 draws | 50-53 | 44 |
| ms per frame, over 4,000 draws | 59-66 | 56 |
| frames over 1.5x the usual | 7-8 % | 2.3 % |
| 5-second periods with a frame over 500 ms | 18-21 | 10 |
| frames drawn in the 6 minutes | 4,400-5,900 | 8,385 |
| texture copies made again in the first 4 minutes | about 6.4 GB | 14 MB |
| memory commit, peak | 37 GB | 33 GB |

Heavier hand-played scenes (a prison yard at 5,500-8,600 draws, the open world) run at about 48-62 ms a frame
(16-21 fps). Identical runs on the test PC differ by 13-18 ms a frame at the same draw count, so single-run frame
times prove little; the memory, re-upload and freeze figures and each switch's own counter line are what is solid.

Where the time goes now (ray tracing off): the graphics card is mostly idle. One thread of the layer, the one that
hands each frame's work to the card, is busy for the whole frame (32-44 ms: buffer refreshes 12-14, copies in the
card's memory 7-8, other list steps about 6, write-watch asks about 4).

## The settings `Play-Scarlett.cmd` uses

All settings are environment variables read at start; `scripts\play-scarlett.ps1` holds the list, in groups:

- cool_fox's launcher set, **without** Series S, `VICTORY_THIN` and the stand-in ambient light, and with the
  keep-the-card-busy loop off (`VICTORY_CARD_AWAKE=0`: it held the card at 97-99 % in menus for no gain).
- `VICTORY_LEVELS_ON_DEMAND=0`: textures keep only the levels their views show. cool_fox's launcher set it to 1,
  which made every texture keep its whole chain on the card. Off: re-uploads near zero, about 4 GB less commit,
  fewer freezes. This was the largest single gain.
- The memory hooks (`-NoHooks` starts without them).
- `VICTORY_POOL_DISCARD=1`, `VICTORY_FILL_TITLE=1`, `VICTORY_PARTIAL_DELTA=1`, `VICTORY_BAKE_DELTA=1`,
  `VICTORY_MAP_LOCK_FINE=1`, `VICTORY_POOL_CAP=1`: unused pool pages given back under low RAM, no read waits for
  fills, unchanged texture levels not uploaded again, the mapping lock taken per range, the texture pool capped.
- `VICTORY_TABLE_MEMO=1`: a descriptor-table entry made ready earlier in a command list is not made ready again
  (81 % of about 54,000 entries a frame; verified against the full path). `VICTORY_LIST_ALIAS_EXACT=1`: a texture
  read through a view in another format follows its source's writes within a list (the base read a stale copy once
  a frame in the title's lighting filter).

## What was added to cool_fox

Always on:
- Unattended runs: `VICTORY_AUTO_INPUT` (a script of pad presses keyed on draws per frame, to pass the start
  screens), a window caption with fps, ms and draws, `VICTORY_FRAME_LOG` (per-frame breakdown by thread, long frames
  listed with their cause), `VICTORY_HANG_WATCH_S`, `VICTORY_PRESENT_SKIP` for runs with the display off.
- Picture: 16-bit float values moved through unsigned-integer views (`VICTORY_HALF_VIEWS`, `VICTORY_D16_BITS`; fixes
  frozen facial animation and a blurred disc in cutscenes), a NaN guard on colour exports (`VICTORY_NAN_GUARD`),
  early depth, pipelines made for the render-target formats actually bound (`VICTORY_TARGET_VARIANTS`), an alias copy
  after an upload earlier in the same list, views keep the title's minimum LOD. 26 compute programs the base refused
  now translate (hair shadow masks, water volumetrics, particle emits, decals).
- Meters (diagnostics builds): uploads by cause, read waits by reason, mapping-lock readers, memory housekeeping,
  texture detail, the game's chunk pool, page kinds by the title's own table, per-pass GPU times
  (`VICTORY_PASS_TIMES=1`, `=2` serial), list calls and draw set-up by part (`VICTORY_LIST_CALLS=1`).

Switches, off by default (the base's behaviour is kept unless a run showed a gain); the ones in the launcher are
described above. Others:
- `VICTORY_PAGE_POOL=1`: textures on pages of reserved resources (about 1.2 GB less texture memory, no frame cost).
  Its page mover `VICTORY_PAGE_MIGRATE` loses the device on the test card and must stay off (`tools/docs/fox-port-V.md`).
- `VICTORY_STAGING_REST_MS`, `VICTORY_STAGING_PACK`, `VICTORY_SEND_SPARE_MB`, `VICTORY_WINDOW_KB`,
  `VICTORY_WINDOW_TAILS`, `VICTORY_HOLDERS`: memory trims; about 1-2 GB less commit, not in the launcher.
- `VICTORY_CLEAN_STAMP`, `VICTORY_ASK_KEPT_EARLY`, `VICTORY_FILL_FROM_PLACED`, `VICTORY_SIGNATURE_LOCAL`,
  `VICTORY_BIND_SAME`, `VICTORY_LIST_BATCH`, `VICTORY_FAULT_NOTE_ONCE`, `VICTORY_BASE_REUSE`, `VICTORY_MAP_DEFER`,
  `VICTORY_REMAP_SAME`, `VICTORY_CHUNK_FINAL`, `VICTORY_BIND_FILTER`, `VICTORY_HOIST_LOADS` and more: built, exact,
  no gain shown (or a loss: `VICTORY_LIST_BATCH`), or untested. `tools/docs` says which.
- Tried and rejected on this base: `VICTORY_THIN=1` (no gain here, twice the long frames), `VICTORY_WINDOWS_BAR_MB`
  (game memory in VRAM through Resizable BAR: no gain while the card is idle, 1.4 GB of VRAM), a release build
  without diagnostics (no faster), `VICTORY_DISPLAY_MODE=2560x1440` (the title still builds its 4K targets),
  `VICTORY_SHADER_LOOP_LIMIT=1024` (black faces), `VICTORY_BVH_COUNT32` (breaks ray tracing), `VICTORY_UNMAP_EXACT`,
  `VICTORY_TEXTURE_RAM_CAPS=0`, `VICTORY_VRAM_RAW_PCT`.

## Known problems

- Bushes and some trees glow purple; purple patches linger for a few seconds in some scenes (present in the base;
  cause not found: `tools/docs/left-out-audit.md` lists the candidates).
- Meshes can stretch for a second or two in busy scenes (seen before today's changes; another fork blames
  `VICTORY_RELAY=2` with `VICTORY_CARD_STEPS=2`; not yet tested here).
- Ray tracing is off by default: the base leaves the title's ray passes out unless `VICTORY_BVH_BUILDER=1`
  (`CS_RaytracingResolve`, about 1,000 programs, and `CS_ScreenProbes_ActivateRays`). Those are all of the draws the
  log reports as dropped. With it on the picture is sharper and better lit, and the card needs about 84 ms a frame
  (`tools/docs/rt-parked-T.md`).
- Heavy scenes sit at 16-21 fps: the layer's submitting thread is the limit (see above).
- Texture pop-in remains, less than before; the card reaches 14-15 GB of 16 in long sessions.
- A first run on an empty shader cache can show missing or black faces.
- In Series X mode the title always renders at 2560x1440 and upscales to 3840x2160 itself.
- Two pipeline creation methods are not implemented (the title falls back to ordinary containers).

## Where things are

- `Play-Scarlett.cmd`, `scripts/play-scarlett.ps1`: the launcher.
- `tools/`: the measuring scripts (`run-auto-coolfox.sh`: hands-off run with watchdog; `run-compare.sh`,
  `run-freezes.sh`, `run-focus.sh`), notes (`tools/docs`: what was measured, tried and rejected; `fox-port-*.md` list
  every ported switch; `cool-fox-willy-review.md` and `left-out-audit.md`: a settings audit and what the layer
  leaves out or approximates), `tools/baseline-coolfox/README.md` (the floor and how to compare against it).
- The older Scarlett tree (soldsawn/victory-nvidia-8gb base) is on branch `scarlett`.

The base's own README follows.

---

# VIctory on an RX 7900 XTX

## Run the codec_d Windows build

Download and extract the Windows x64 ZIP from the [private releases](https://github.com/codec-d/cool_fox/releases/latest). Open PowerShell in the extracted folder and point the launcher at your own complete Xbox dump.

```powershell
$env:VICTORY_GAME_DIR = 'D:\Games\MyDump'
.\Play-VIctory.cmd pareto-residency
```

The launcher selects Series S and the RTX-tested residency settings. It builds its shader cache locally under `%LOCALAPPDATA%\VIctory\cache-pareto`. Set `VICTORY_CACHE_DIR` to reuse an existing cache outside the checkout. Windows x64, an AVX2 CPU, and a Direct3D 12 GPU with shader model 6.6 and mesh shader support are required.

To build from this checkout, install the pinned Rust toolchain and the MSVC C++ build tools with a Windows SDK, then run `cargo build --release --locked -p victory`. The same launcher finds `target\release\victory.exe`. The sections below describe the upstream AMD fork and its separate launcher.

A fork of [VIctory](https://github.com/Marvisak/VIctory), a compatibility layer
that runs an Xbox Series title natively on Windows from files the user supplies.
This one is about **speed on AMD RDNA 3**: the world at 30 frames a second on
one PC, without giving up the picture. It is not there yet.

**This repository contains no game files, code or assets.** You need your own
copy of the game. It is not affiliated with or endorsed by the game's developer
or publisher.

## Where it stands (2026-11-25)

Tested on one machine only: Windows 11, Intel i9-12900K, 64 GB RAM, Radeon
RX 7900 XTX, 1440p, the Series X profile.

| | |
|---|---|
| Menus, videos, loading screens | 30 fps |
| Prologue, indoors | 50-75 ms a frame (13-20 fps) |
| First mission with the boat | 46-53 ms (19-22 fps) |
| Street chase with many cars and people | 100-165 ms (6-10 fps), single frames far longer |
| Outdoors on foot, the camp | 60-90 ms |
| Flying | 42-57 ms |
| Longest session so far | 41 minutes, no crash |
| Memory | 40-46 GB of private memory: **64 GB of RAM recommended** |

It started at 280 ms a frame in the same scenes two days earlier.

What is known to be wrong:

- Trees, plants and far buildings flicker outdoors in some places.
- Rooms with little direct light are too dark without `-RayTracing`.
- Textures of what just came into view appear late while the title streams.
- The first run with a new driver or a new build stutters while the driver makes
  pipelines (`victory warm` makes them ahead).
- Recording a frame in the world makes the title's own watchdog end it.

## How to run it

You need Windows 10/11 x64, a CPU with AVX2, a Direct3D 12 GPU with shader
model 6.6 and mesh shaders, Rust (the toolchain is pinned), the MSVC build
tools, and your own dump of the title.

```powershell
git clone https://github.com/avatarmanz/VIctory-7900XTX
cd VIctory-7900XTX
copy victory.example.toml victory.toml      # then set your paths, outside this folder
cargo build --locked --profile diag -p victory --features diagnostics
scripts\play.ps1 -Exe <path of your copy's executable>
```

`scripts\play.ps1 -Safe` leaves out the newest, faster ways; try it first when
something is wrong. `-RayTracing` turns the title's ray-traced indirect light on.

Before the first run, ahead of time (both can be stopped and go on later):

```powershell
target\diag\victory.exe shaders <the shader archives>   # every shader translated, minutes
target\diag\victory.exe warm --used                     # pipelines earlier runs drew with, for a new driver or build
```

Commands, debugging switches and upstream's own description:
[docs/README-fxceless.md](docs/README-fxceless.md). Every `VICTORY_...` switch is
described in a comment where it is read.

## What is different here

In short; the reasons and the measurements are in [docs/findings.md](docs/findings.md).

- **List calls are written down, not carried out, on the title's threads**
  (`VICTORY_THIN`): eight threads of ours translate them.
- **Submissions are prepared by a thread of ours and joined**
  (`VICTORY_ASYNC_SUBMIT`, `VICTORY_JOIN_PARTS`), with what can be spread over
  threads spread: refreshes, compares (AVX2), descriptor records, landings.
- **What shaders wrote comes back beside the write guards**, by the dwords that
  changed, on a thread of its own (`VICTORY_RELAY`, `VICTORY_LAND`).
- **Buffer fills that need bytes still on the card are done on the card**
  (`VICTORY_CARD_STEPS`) instead of waiting for it.
- **The title's writes are caught more cheaply**: one view opened at a fault,
  blocks that rest, places in the title's code that look before they write.
- **Textures go up on a copy queue of their own** and a slot shows one only after
  its upload ran, which also ended a device loss that came every 15 minutes or so.
- **Boats float**: textures that shaders write and the title's CPU reads go back
  to title memory (from fxceless and Geckomacabre, on by default here).
- **Frame record and replay** with per-pass times and shader variants compared
  turn by turn ([docs/replay.md](docs/replay.md)).

## Credits

This is a tree of other people's work with more on top. The history is kept, and
commits taken from elsewhere name the commit and its author.

- **[VIctory](https://github.com/Marvisak/VIctory)** by Marvisak and its
  contributors: the loader, the runtime services, the Direct3D 12 back end and
  the shader recompiler.
- **[fxceless/victory-fork](https://github.com/fxceless/victory-fork)**: the fork
  this one started from, and the people whose pull requests it carries.
- **[DexNom/VIctory](https://github.com/DexNom/VIctory)**: ray tracing fixes,
  cube gradients, sparse textures.
- **[Belobug/VIctory-Belobug-AMD](https://github.com/Belobug/VIctory-Belobug-AMD)**:
  the ambient light idea, pipeline workers, and work by romainhedouin.
- **[FQY55kai/ZJOU-VICTORY](https://github.com/FQY55kai/ZJOU-VICTORY)**.
- **[myokex/victory-amd](https://github.com/myokex/victory-amd)**: the guard over
  every view of a page (with Spexivus, joroyt and tightfall), and Joshua Odom's
  reads into title memory.
- **Geckomacabre** and **cxy0072019-sketch**: the texture read-back.
- **[XCL](https://github.com/ivall/xcl)** (GPL-2.0-only) was read for how the same
  title behaves under another layer; no code of it is here.
- This fork's changes were made with [Claude Code](https://claude.com/claude-code).

## License

GPL-3.0-or-later, as VIctory. See [LICENSE](LICENSE); the one file that is not
covered is named in [docs/asset-policy.md](docs/asset-policy.md).
