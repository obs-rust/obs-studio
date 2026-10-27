# Tier 2 validation (docs/rust-port/testing-policy.md) on Windows. One build
# makes the cmocka tests and both obs.dll files, libobs (C) and
# libobs-c-reference (#1065); it runs the unchanged C tests against each and
# fails unless both export exactly the same symbols. Mirrors
# rust/tools/linux-validate/validate.sh.
#
# Prerequisites: cmake, Visual Studio, and cmocka (vcpkg install
# cmocka:x64-windows). The windows-ci-x64 preset downloads the pre-built
# obs-deps during configure.
#
# -Repeat N reruns the tests (or those matching -Tests REGEX) up to N times in
# both builds after the normal ctest run, stopping at the first failure
# (ctest --repeat until-fail:N), e.g.
#   validate.ps1 -CMockaPrefix ... -Repeat 100 -Tests 'test_threading|test_task'
param(
  [string]$CMockaPrefix,
  [ValidateRange(0, [int]::MaxValue)][int]$Repeat = 0,
  [string]$Tests
)

$ErrorActionPreference = 'Stop'

if (-not $CMockaPrefix) {
  throw 'Pass -CMockaPrefix <vcpkg installed/x64-windows directory>'
}
if ($Tests -and $Repeat -eq 0) {
  throw '-Tests only applies with -Repeat'
}
$CMockaPrefix = $CMockaPrefix -replace '\\', '/'

# OBS_CTEST_JOBS: ctest parallelism for each suite (default 1); CI sets it
# (#2138). Tests that must not overlap carry RUN_SERIAL or RESOURCE_LOCK.
$ctestJobs = @('-j', $(if ($env:OBS_CTEST_JOBS) { $env:OBS_CTEST_JOBS } else { '1' }))

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot '../../..')
Set-Location $RepoRoot

# cmocka.dll must be on PATH for the test executables.
$env:PATH = "$CMockaPrefix/bin;$env:PATH"

# ctest reports a crashed test only as "Exception: SegFault". WER saves a
# dump of the *original* faulting process: #2543 did not reproduce in 20
# isolated cdb reruns. The cdb rerun remains a fallback for tests without dumps.
function Show-CrashStacks([string]$build, [string]$dumpDirectory) {
  $failedLog = Join-Path $build 'Testing/Temporary/LastTestsFailed.log'
  if (-not (Test-Path $failedLog)) { return }
  $kits = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits/10/Debuggers/x64/cdb.exe'
  if (-not (Test-Path $kits)) {
    Write-Host 'cdb.exe not found; no crash stacks'
    return
  }
  $qt = Get-ChildItem -Path (Join-Path $RepoRoot '.deps') -Recurse -Filter 'Qt6Core.dll' -ErrorAction SilentlyContinue |
    Select-Object -First 1
  $runPath = $env:PATH
  if ($qt) { $runPath = "$($qt.DirectoryName);$runPath" }
  foreach ($line in Get-Content $failedLog) {
    $name = ($line -split ':', 2)[1]
    if (-not $name) { continue }
    $exe = Get-ChildItem -Path $build -Recurse -Filter "$name.exe" | Select-Object -First 1
    if (-not $exe) { continue }
    Write-Host "== crash stack: $name"
    $env:PATH = $runPath
    $dump = if ($dumpDirectory) {
      Get-ChildItem -Path $dumpDirectory -Filter "$name*.dmp" -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    }
    if ($dump) {
      Write-Host "analyzing original crash dump: $($dump.Name)"
      & $kits -z $dump.FullName -lines -c '.ecxr;kn 40;q' 2>&1 |
        Select-Object -Last 100 | ForEach-Object { Write-Host $_ }
    } else {
      Write-Host 'no crash dump; rerunning once under cdb'
      & $kits -g -G -lines -c '.ecxr;kn 40;q' $exe.FullName 2>&1 |
        Select-Object -Last 80 | ForEach-Object { Write-Host $_ }
    }
  }
}

function Find-Dumpbin {
  $cmd = Get-Command dumpbin.exe -ErrorAction SilentlyContinue
  if ($cmd) { return $cmd.Source }
  $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
  $found = & $vswhere -latest -products * -find 'VC/Tools/MSVC/**/bin/Hostx64/x64/dumpbin.exe' | Select-Object -First 1
  if ($LASTEXITCODE -ne 0) { throw "vswhere failed ($LASTEXITCODE)" }
  if (-not $found) { throw 'dumpbin.exe not found' }
  return $found
}

function Get-Exports($dll) {
  $dumpbin = Find-Dumpbin
  $out = & $dumpbin /exports $dll.FullName
  if ($LASTEXITCODE -ne 0) { throw "dumpbin failed ($LASTEXITCODE)" }
  # Export rows: ordinal, hint, RVA, name (optionally followed by "= ...").
  $names = $out |
    ForEach-Object { if ($_ -match '^\s+\d+\s+[0-9A-Fa-f]+\s+[0-9A-Fa-f]{8}\s+(\S+)') { $Matches[1] } } |
    Sort-Object
  if (-not $names) { throw "no exports parsed from $($dll.FullName)" }
  return $names
}

$build = 'build_rust'
Write-Host '== configure + build (libobs and libobs-c-reference)'
cmake --preset windows-ci-x64 -B $build `
  -DENABLE_UNIT_TESTS=ON -DOBS_BUILD_C_REFERENCE=ON -DOBS_LIBOBS_BACKEND=HYBRID `
  -DENABLE_FRONTEND=OFF -DENABLE_SCRIPTING=OFF -DENABLE_BROWSER=OFF `
  -DENABLE_WEBSOCKET=OFF `
  "-DCMAKE_PREFIX_PATH=$CMockaPrefix"
if ($LASTEXITCODE -ne 0) { throw "cmake configure failed ($LASTEXITCODE)" }

cmake --build $build --config RelWithDebInfo `
  --target libobs libobs-c-reference cmocka-tests libobs-d3d11 libobs-opengl
if ($LASTEXITCODE -ne 0) { throw "cmake build failed ($LASTEXITCODE)" }

# obs.dll twice: libobs (C, what the tests link) and its Rust twin in
# $build/libobs-c-reference (#1065), same name.
$cDir = (Resolve-Path (Join-Path $build 'libobs-c-reference')).Path
$rustDll = Get-ChildItem -Path $build -Recurse -Filter obs.dll |
  Where-Object { $_.FullName -match 'libobs' -and -not $_.FullName.StartsWith($cDir) } | Select-Object -First 1
$cDll = Get-ChildItem -Path $cDir -Recurse -Filter obs.dll | Select-Object -First 1
if (-not $cDll) { throw "obs.dll (C) not found under $build" }
if (-not $rustDll) { throw "obs.dll (with Rust) not found under $build" }

Write-Host '== libobs exported symbols, C vs Rust'
$off = Get-Exports $cDll
$on = Get-Exports $rustDll
$off | Set-Content exports-OFF.txt
$on | Set-Content exports-ON.txt
$diff = Compare-Object $off $on
if ($diff) {
  $diff | Format-Table | Out-String | Write-Host
  throw 'FAIL: exported symbols differ between libobs and libobs-c-reference'
}
Write-Host "IDENTICAL ($($on.Count) symbols)"

# The test executables sit apart from obs.dll and its runtime DLLs
# (w32-pthreads, pre-built obs-deps FFmpeg), so PATH decides which obs.dll
# they load. A copy beside a test would win over PATH and hide the Rust one.
$besideTests = Get-ChildItem -Path (Join-Path $build 'test') -Recurse -Filter obs.dll -ErrorAction SilentlyContinue
if ($besideTests) { throw "obs.dll next to the tests would shadow PATH: $($besideTests.FullName -join ', ')" }
$pthreads = Get-ChildItem -Path $build -Recurse -Filter w32-pthreads.dll | Select-Object -First 1
if (-not $pthreads) { throw "w32-pthreads.dll not found under $build" }
$avcodec = Get-ChildItem -Path (Join-Path $RepoRoot '.deps') -Recurse -Filter 'avcodec-*.dll' |
  Select-Object -First 1
if (-not $avcodec) { throw 'obs-deps avcodec DLL not found under .deps' }

foreach ($mode in 'OFF', 'ON') {
  $dll = if ($mode -eq 'ON') { $rustDll } else { $cDll }
  $savedPath = $env:PATH
  $env:PATH = "$($dll.DirectoryName);$($pthreads.DirectoryName);$($avcodec.DirectoryName);$env:PATH"

  Write-Host "== [$mode] ctest against $($dll.FullName)"
  ctest --test-dir $build -C RelWithDebInfo --output-on-failure @ctestJobs
  $ctestExit = $LASTEXITCODE

  if ($ctestExit -eq 0 -and $Repeat -gt 0) {
    $repeatArgs = @('--repeat', "until-fail:$Repeat")
    if ($Tests) { $repeatArgs += @('-R', $Tests) }
    Write-Host "== [$mode] ctest $($repeatArgs -join ' ')"
    ctest --test-dir $build -C RelWithDebInfo --output-on-failure @repeatArgs
    $ctestExit = $LASTEXITCODE
  }

  if ($ctestExit -ne 0) { Show-CrashStacks $build }
  $env:PATH = $savedPath
  if ($ctestExit -ne 0) { throw "ctest failed ($ctestExit)" }
}

# Build the Cargo cores using the same native architecture and dependency bundle
# as the baseline. Cargo builds the Rust archive outside build.rs to avoid a
# nested Cargo invocation and to select an explicit, freshly built artifact.
$nativeTarget = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') {
  'aarch64-pc-windows-msvc'
} else {
  'x86_64-pc-windows-msvc'
}
$nativeOutput = Join-Path $RepoRoot "$build/cargo-native-core"
$nativeRust = Join-Path $RepoRoot "$build/cargo-native-rust"
$nativeTests = Join-Path $RepoRoot "$build/cargo-native-tests"
$nativeDependencies = Split-Path $avcodec.DirectoryName
soldr cargo build -p libobs-rust --release --target $nativeTarget --target-dir $nativeRust
if ($LASTEXITCODE -ne 0) { throw "Cargo Rust archive failed ($LASTEXITCODE)" }
$nativeArchive = Join-Path $nativeRust "$nativeTarget/release/libobs_rust.lib"
$env:OBS_BUILD_WINDOWS_DEPENDENCIES = $nativeDependencies
$env:OBS_BUILD_WINDOWS_TARGET = $nativeTarget
$env:OBS_BUILD_WINDOWS_OUTPUT = $nativeOutput
$env:OBS_BUILD_WINDOWS_RUST_ARCHIVE = $nativeArchive
$env:OBS_BUILD_WINDOWS_PCH = '1'
$env:OBS_BUILD_WINDOWS_REFERENCE = $cDll.FullName
$env:OBS_BUILD_WINDOWS_EXPORT_TOOL = Find-Dumpbin
$env:OBS_BUILD_WINDOWS_GRAPHICS = '1'
$referenceRenderers = @{}
foreach ($renderer in 'D3D11', 'OPENGL') {
  $target = if ($renderer -eq 'D3D11') { 'libobs-d3d11' } else { 'libobs-opengl' }
  $reference = Get-ChildItem -Path (Join-Path $build $target) -Recurse -Filter "$target.dll" | Select-Object -First 1
  if (-not $reference) { throw "CMake renderer $target.dll not found" }
  $referenceRenderers[$renderer] = $reference.FullName
  Set-Item "env:OBS_BUILD_WINDOWS_$($renderer)_REFERENCE" $reference.FullName
}
soldr cargo test -p obs-studio-rust --features libobs-integration `
  --target $nativeTarget --target-dir $nativeTests --test obs-tests-app `
  native_libobs::windows_tests::core_builds_from_the_canonical_manifest -- --exact
if ($LASTEXITCODE -ne 0) { throw "Cargo Windows core contract failed ($LASTEXITCODE)" }

$env:OBS_SYS_NATIVE_DEPENDENCIES = $nativeDependencies
$env:OBS_BUILD_RUST_ARCHIVE = $nativeArchive
$env:OBS_SYS_UNITY = '1'
$env:OBS_SYS_PCH = '0'
soldr cargo check -p obs-sys --features bundled --target $nativeTarget --target-dir $nativeTests
if ($LASTEXITCODE -ne 0) { throw "bundled Windows obs-sys failed ($LASTEXITCODE)" }

$nativeArtifacts = Get-Content (Join-Path $nativeOutput 'windows-artifacts.json') -Raw | ConvertFrom-Json
$nativeSymbols = Get-Content (Join-Path $nativeOutput 'windows-symbols.json') -Raw | ConvertFrom-Json
Write-Host "Cargo IDENTICAL ($($nativeSymbols.Count) symbols), target=$($nativeArtifacts.target), unity=$($nativeArtifacts.unity), pch=$($nativeArtifacts.pch)"
$testPlanJson = ctest --test-dir $build -C RelWithDebInfo --show-only=json-v1
if ($LASTEXITCODE -ne 0) { throw 'could not inspect unchanged CTest runtime paths' }
$testPlan = ($testPlanJson -join [Environment]::NewLine) | ConvertFrom-Json
if (-not $testPlan.tests.Count) { throw 'no unchanged native tests were registered' }
# A per-test PATH override, a prepended directory or a working directory with
# obs.dll could hide the selected Cargo core. Keep Qt's normal PATH prepends,
# but prove they cannot shadow the core before using the unmodified tests.
foreach ($test in $testPlan.tests) {
  $executableDirectory = Split-Path $test.command[0]
  if ($executableDirectory -and (Test-Path (Join-Path $executableDirectory 'obs.dll'))) {
    throw "test $($test.name) has a shadowing executable-directory obs.dll"
  }
  foreach ($property in $test.properties) {
    if ($property.name -eq 'ENVIRONMENT') {
      foreach ($value in $property.value) {
        if ($value -match '^PATH=') { throw "test $($test.name) overrides PATH" }
      }
    }
    if ($property.name -eq 'WORKING_DIRECTORY' -and (Test-Path (Join-Path $property.value 'obs.dll'))) {
      throw "test $($test.name) has a shadowing working-directory obs.dll"
    }
    if ($property.name -eq 'ENVIRONMENT_MODIFICATION') {
      foreach ($value in $property.value) {
        if ($value -match '^PATH=path_list_prepend:(.*)$') {
          foreach ($directory in $Matches[1] -split ';') {
            if (Test-Path (Join-Path $directory 'obs.dll')) {
              throw "test $($test.name) prepends a shadowing obs.dll"
            }
          }
        } elseif ($value -match '^PATH=' -and $value -notmatch '^PATH=path_list_append:') {
          throw "test $($test.name) changes PATH unexpectedly: $value"
        }
      }
    }
  }
}

foreach ($selected in $nativeArtifacts.reference, $nativeArtifacts.core) {
  $dll = Get-Item $selected
  $savedPath = $env:PATH
  $dumpDirectory = $null
  $dumpKey = 'HKCU:\Software\Microsoft\Windows\Windows Error Reporting\LocalDumps\test_scene_canvas_characterization.exe'
  try {
    if (-not (Test-Path $dumpKey)) {
      $dumpDirectory = Join-Path $nativeOutput 'crash-dumps'
      New-Item -ItemType Directory -Path $dumpDirectory -Force | Out-Null
      New-Item -Path $dumpKey -Force | Out-Null
      New-ItemProperty -Path $dumpKey -Name DumpFolder -Value $dumpDirectory -PropertyType ExpandString | Out-Null
      New-ItemProperty -Path $dumpKey -Name DumpType -Value 2 -PropertyType DWord | Out-Null
      New-ItemProperty -Path $dumpKey -Name DumpCount -Value 1 -PropertyType DWord | Out-Null
    }
    $nativePthreads = Split-Path $nativeArtifacts.pthreads
    $env:PATH = "$($dll.DirectoryName);$nativePthreads;$($avcodec.DirectoryName);$savedPath"
    Write-Host "== Cargo loader identity: $($dll.FullName)"
    & $nativeArtifacts.loader_probe $dll.FullName
    if ($LASTEXITCODE -ne 0) { throw "Cargo core loader identity failed ($LASTEXITCODE)" }
    # Negative control: same selected DLL, deliberately wrong expected file.
    & $nativeArtifacts.loader_probe $cDll.FullName
    if ($LASTEXITCODE -ne 14) { throw "Cargo loader negative control failed ($LASTEXITCODE)" }
    foreach ($renderer in 'D3D11', 'OPENGL') {
      $rendererPath = $nativeArtifacts.renderers.$renderer
      if (-not $rendererPath) { throw "missing Cargo $renderer artifact" }
      Write-Host "== Cargo $renderer module identity and version resource"
      & $nativeArtifacts.graphics_probe $rendererPath $renderer
      if ($LASTEXITCODE -ne 0) { throw "Cargo $renderer probe failed ($LASTEXITCODE)" }
      $wrongRenderer = if ($renderer -eq 'D3D11') { 'OPENGL' } else { 'D3D11' }
      & $nativeArtifacts.graphics_probe $rendererPath $wrongRenderer
      if ($LASTEXITCODE -ne 14) { throw "Cargo $renderer identity negative control failed ($LASTEXITCODE)" }
      & $nativeArtifacts.graphics_probe $referenceRenderers[$renderer] $renderer
      if ($LASTEXITCODE -ne 0) { throw "CMake $renderer probe failed ($LASTEXITCODE)" }
    }
    Write-Host "== Cargo [$($dll.Directory.Name)] unchanged ctest suite"
    ctest --test-dir $build -C RelWithDebInfo --output-on-failure --no-tests=error @ctestJobs
    $ctestExit = $LASTEXITCODE
    if ($ctestExit -eq 0 -and $Repeat -gt 0) {
      $repeatArgs = @('--repeat', "until-fail:$Repeat")
      if ($Tests) { $repeatArgs += @('-R', $Tests) }
      ctest --test-dir $build -C RelWithDebInfo --output-on-failure --no-tests=error @repeatArgs
      $ctestExit = $LASTEXITCODE
    }
    if ($ctestExit -ne 0) {
      Show-CrashStacks $build $dumpDirectory
      throw "Cargo core CTest failed ($ctestExit)"
    }
  } finally {
    if ($dumpDirectory) { Remove-Item -Path $dumpKey -Force }
    $env:PATH = $savedPath
  }
}
