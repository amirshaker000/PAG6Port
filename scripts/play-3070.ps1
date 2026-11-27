# Play-Unified: Keel62155/VIctoryUnified (cool_fox for 8 GB NVIDIA, measured 18-21 fps on an RTX 4060 8 GB),
# fast build, its scripts\play-nvidia-8gb.cmd settings, our paths (all on E:), 1080p.
#   .\Play-Unified.ps1            translate shaders once per build, then play
#   .\Play-Unified.ps1 -More @{ VICTORY_LOG = 'info' }
param([hashtable]$More = @{})
$root  = 'E:\Game\GTA6.Xbox.v1.0.2.1-CYBERLEEK'
$repo  = "$root\GTAVI-Repos\unified"
$exe   = "$repo\target\fast\victory.exe"
$gameD = $root
$cache = "$root\Project\unified-home"
$data  = "$cache\data"
$logs  = "$root\Project\evidence"
$fsr   = 'E:\SteamLibrary\steamapps\common\Arc Raiders\Engine\Plugins\Runtime\AMD\FSR\Source\fidelityfx-sdk\Kits\FidelityFX\signedbin\amd_fidelityfx_upscaler_dx12.dll'
New-Item -ItemType Directory -Force -Path $cache, $data, $logs | Out-Null
if (Get-Process victory -ErrorAction SilentlyContinue) { throw 'victory.exe is already running: one game at a time.' }

Get-ChildItem Env: | Where-Object { $_.Name -like 'VICTORY_*' } | ForEach-Object { Remove-Item "Env:$($_.Name)" }
$set = [ordered]@{
    VICTORY_GAME_DIR = $gameD; VICTORY_CACHE_DIR = $cache; VICTORY_DATA_DIR = $data
    # scripts\play-nvidia-8gb.cmd (measured on an RTX 4060 8 GB)
    VICTORY_AMBIENT_AIR = '1.5'; VICTORY_TEXTURE_NO_WAIT = '1'; VICTORY_RELAY = '2'; VICTORY_WIDE_BACK = '1'
    VICTORY_CARD_REHASH_RUNS = '1'; VICTORY_JOIN_COPIES = '1'; VICTORY_INDIRECT_WINDOW = '1'; VICTORY_RELAY_COPY = '1'
    VICTORY_CARD_STEPS = '2'; VICTORY_INDIRECT_BATCH = '2'; VICTORY_MAP_THREADS = '1'; VICTORY_CARD_AWAKE = '1'
    VICTORY_SHOW_AFTER = '1'; VICTORY_AMBIENT = '0.8'; VICTORY_THIN = '1'; VICTORY_JOIN_PARTS = '8'
    VICTORY_UPLOAD_AHEAD = '1'; VICTORY_CARD_BUFFERS_MIN_KB = '64'; VICTORY_LEVELS_ON_DEMAND = '0'
    VICTORY_WAVE64_PAIRED = '1'; VICTORY_F16_NEAREST = '1'; VICTORY_CARD_BUFFERS_MB = '1024'
    VICTORY_WINDOWS_BAR_MB = '512'; VICTORY_E5_TARGETS = 'r11g11b10'; VICTORY_THIN_THREADS = '6'
    VICTORY_FSR_DLL = $fsr
    # this PC
    VICTORY_DISPLAY_MODE = '1920x1080'; VICTORY_PIPELINE_BOOK = '1'
    # our mouse port (cursor pinned to centre feeds the pad stick / the GameInput mouse device)
    VICTORY_MOUSE = '1'; VICTORY_MOUSE_DEVICE = '1'
    # willy/foxhound frame-rate unlock: the title's 30 fps setting patched to 60
    # VICTORY_FRAME_INTERVAL_MS = '16.667'  # off: doubles the title's work (300+ ms vs 85-108 ms)
    # oplite 50bd954e picture fixes: ray probes with RT off (violet vegetation), character bake time (faces)
    # VICTORY_ACTIVATE_RAYS = '1'; VICTORY_RT_RESOLVE = '1'; VICTORY_BAKE_TEXTURES_MS = '10000'  # off: ray probes and bake waits cost 3x frame time
    VICTORY_LOG = 'warn'
    # WASDeKeys memory caps (prelude recycling, staging idle cap): no 0x8007000E commit OOM
    VICTORY_MAX_PRELUDES = '32'; VICTORY_STAGING_KEPT = '4'
    # Ray tests answer empty (the builder is off): no violet/purple glow, about 10 ms a frame less (trustno1/megafork; = Yaekai PROBE_RAYS)
    # VICTORY_BVH_MISS = '1'  # translator switch: changes every pipeline fingerprint (full 1 h re-warm)
    # Yaekai/victory-scarlett-3070: broken water/heat-haze and decal-defrag pipelines skipped; NVIDIA heap cap
    VICTORY_SKIP_PIPELINES = '02384a60fda6f2d0,731a39171b568298'; VICTORY_DYNAMIC_SLOTS = '524288'
    # 8 GB shared with desktop apps: the 1536/1024 MB reservations filled the card (7736 of 8192 MB, black screen)
    VICTORY_TEXTURE_BUDGET_MB = '4096'
}
# DLSS Super Resolution (foxhound port): on when the Streamline DLLs are in Project\streamline
$sl = "$root\Project\streamline"
if (Test-Path "$sl\sl.dlss.dll") {
    $set.VICTORY_DLSS = '1'; $set.VICTORY_DLSS_MODE = 'quality'; $set.VICTORY_STREAMLINE_DIR = $sl
}
foreach ($k in $More.Keys) { $set[$k] = [string]$More[$k] }
foreach ($k in $set.Keys) { Set-Item -Path "Env:$k" -Value $set[$k] }
if (-not (Test-Path -LiteralPath $fsr)) { Write-Host 'WARNING: FSR DLL missing; the picture may stay black on NVIDIA.' }

Set-Location $repo
$ErrorActionPreference = 'Continue'
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$mark = "$cache\shaders-done-$((Get-Item $exe).LastWriteTime.Ticks).txt"
if (-not (Test-Path $mark)) {
    Write-Host 'Translating the game shaders ahead (once per build, ~10-20 min)...'
    & $exe shaders "$gameD\shaders.rpf" 2>&1 | Out-File "$logs\unified-shaders-$stamp.log" -Encoding utf8
    'done' | Out-File $mark
}
# Full pipeline warm-up once per build: every pipeline in the book is made before playing,
# so the game does not compile them mid-scene (first-session stutter and missing-shader look)
$wmark = "$cache\pipelines-warm-$((Get-Item $exe).LastWriteTime.Ticks).txt"
if ((Test-Path "$cache\pipelines\known-pipelines.bin") -and -not (Test-Path $wmark)) {
    Write-Host 'Making every known pipeline ahead (once per build, can take up to an hour)...'
    & $exe warm --jobs 8 2>&1 | Out-File "$logs\unified-warm-$stamp.log" -Encoding utf8
    'done' | Out-File $wmark
}
$log = "$logs\unified-$stamp.log"
Write-Host "Playing. Log: $log"
& $exe run "$gameD\game_scarlett_master.exe" --console series-s --execute 2>&1 | Out-File $log -Encoding utf8
