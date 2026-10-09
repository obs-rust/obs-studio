# Tier 2 validation (docs/rust-port/testing-policy.md) on Windows. Builds libobs
# and the cmocka tests with ENABLE_RUST_LIBOBS=OFF and =ON, runs the unchanged
# C tests in both, and fails unless both builds export exactly the same
# symbols from obs.dll. Mirrors rust/tools/linux-validate/validate.sh.
#
# Prerequisites: cmake, Visual Studio, and cmocka (vcpkg install
# cmocka:x64-windows). The windows-ci-x64 preset downloads the pre-built
# obs-deps during configure.
param([string]$CMockaPrefix)

$ErrorActionPreference = 'Stop'

if (-not $CMockaPrefix) {
  throw 'Pass -CMockaPrefix <vcpkg installed/x64-windows directory>'
}
$CMockaPrefix = $CMockaPrefix -replace '\\', '/'

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot '../../..')
Set-Location $RepoRoot

# cmocka.dll must be on PATH for the test executables.
$env:PATH = "$CMockaPrefix/bin;$env:PATH"

function Find-Dumpbin {
  $cmd = Get-Command dumpbin.exe -ErrorAction SilentlyContinue
  if ($cmd) { return $cmd.Source }
  $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
  $found = & $vswhere -latest -products * -find 'VC/Tools/MSVC/**/bin/Hostx64/x64/dumpbin.exe' | Select-Object -First 1
  if ($LASTEXITCODE -ne 0) { throw "vswhere failed ($LASTEXITCODE)" }
  if (-not $found) { throw 'dumpbin.exe not found' }
  return $found
}

foreach ($mode in 'OFF', 'ON') {
  $build = "build_rust_$mode"
  Write-Host "== [$mode] configure + build"
  cmake --preset windows-ci-x64 -B $build `
    -DENABLE_UNIT_TESTS=ON `
    "-DENABLE_RUST_LIBOBS=$mode" `
    -DENABLE_FRONTEND=OFF -DENABLE_SCRIPTING=OFF -DENABLE_BROWSER=OFF `
    -DENABLE_WEBSOCKET=OFF `
    "-DCMAKE_PREFIX_PATH=$CMockaPrefix"
  if ($LASTEXITCODE -ne 0) { throw "cmake configure failed ($LASTEXITCODE)" }

  cmake --build $build --config RelWithDebInfo `
    --target libobs test_bitstream test_darray test_serializer test_os_path test_svt_av1 test_avc
  if ($LASTEXITCODE -ne 0) { throw "cmake build failed ($LASTEXITCODE)" }

  $dll = Get-ChildItem -Path $build -Recurse -Filter obs.dll |
    Where-Object { $_.FullName -match 'libobs' } | Select-Object -First 1
  if (-not $dll) { throw "obs.dll not found under $build" }

  # The test executables sit apart from obs.dll and its runtime DLLs
  # (w32-pthreads, pre-built obs-deps FFmpeg), so put those on PATH.
  $pthreads = Get-ChildItem -Path $build -Recurse -Filter w32-pthreads.dll | Select-Object -First 1
  if (-not $pthreads) { throw "w32-pthreads.dll not found under $build" }
  $avcodec = Get-ChildItem -Path (Join-Path $RepoRoot '.deps') -Recurse -Filter 'avcodec-*.dll' |
    Select-Object -First 1
  if (-not $avcodec) { throw 'obs-deps avcodec DLL not found under .deps' }
  $savedPath = $env:PATH
  $env:PATH = "$($dll.DirectoryName);$($pthreads.DirectoryName);$($avcodec.DirectoryName);$env:PATH"

  Write-Host "== [$mode] ctest"
  ctest --test-dir $build -C RelWithDebInfo --output-on-failure
  $ctestExit = $LASTEXITCODE
  $env:PATH = $savedPath
  if ($ctestExit -ne 0) { throw "ctest failed ($ctestExit)" }

  $dumpbin = Find-Dumpbin
  $out = & $dumpbin /exports $dll.FullName
  if ($LASTEXITCODE -ne 0) { throw "dumpbin failed ($LASTEXITCODE)" }

  # Export rows: ordinal, hint, RVA, name (optionally followed by "= ...").
  $names = $out |
    ForEach-Object { if ($_ -match '^\s+\d+\s+[0-9A-Fa-f]+\s+[0-9A-Fa-f]{8}\s+(\S+)') { $Matches[1] } } |
    Sort-Object
  if (-not $names) { throw "no exports parsed from $($dll.FullName)" }
  $names | Set-Content "exports-$mode.txt"
}

Write-Host '== libobs exported symbols, OFF vs ON'
$off = Get-Content exports-OFF.txt
$on = Get-Content exports-ON.txt
$diff = Compare-Object $off $on
if ($diff) {
  $diff | Format-Table | Out-String | Write-Host
  throw 'FAIL: exported symbols differ between OFF and ON'
}
Write-Host "IDENTICAL ($($on.Count) symbols)"
