# Build GoBlock on Windows (MinGW + Ninja + vcpkg)
# Usage: .\build.ps1 [-Clean] [-Release]
param(
    [switch]$Clean,
    [switch]$Release
)

$ErrorActionPreference = "Stop"
# Native tools write warnings to stderr; PowerShell 7 would turn those into
# terminating errors under "Stop", so marshal native output through Out-String.
function Invoke-Native([scriptblock]$cmd) {
    $out = & $cmd 2>&1 | Out-String
    if ($out) { Write-Host $out.TrimEnd() }
    return $LASTEXITCODE
}

$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$buildDir = Join-Path $root "build"

# --- Toolchain (scoop) -------------------------------------------------------
$extraPaths = @(
    "$env:USERPROFILE\scoop\shims",
    "$env:USERPROFILE\scoop\apps\mingw\current\bin"
)
foreach ($p in $extraPaths) {
    if ((Test-Path $p) -and ($env:Path -notlike "*$p*")) { $env:Path = "$p;$env:Path" }
}

foreach ($tool in @("cmake", "ninja", "g++")) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
        throw "'$tool' not found. Install with: scoop install cmake ninja mingw"
    }
}

# --- vcpkg -------------------------------------------------------------------
$vcpkgRoot = $env:VCPKG_ROOT
if (-not $vcpkgRoot) {
    $vcpkgRoot = Join-Path $env:LOCALAPPDATA "vcpkg"
}
if (-not (Test-Path (Join-Path $vcpkgRoot "scripts\buildsystems\vcpkg.cmake"))) {
    throw "vcpkg not found at '$vcpkgRoot'. Set VCPKG_ROOT or run: git clone https://github.com/microsoft/vcpkg `"$vcpkgRoot`" && `"$vcpkgRoot\bootstrap-vcpkg.bat`" -disableMetrics"
}

if ($Clean -and (Test-Path $buildDir)) { Remove-Item -Recurse -Force $buildDir }

$buildType = if ($Release) { "Release" } else { "Debug" }

$cmakeArgs = @(
    "-S", $root, "-B", $buildDir, "-G", "Ninja",
    "-DCMAKE_BUILD_TYPE=$buildType",
    "-DCMAKE_C_COMPILER=gcc", "-DCMAKE_CXX_COMPILER=g++",
    "-DCMAKE_TOOLCHAIN_FILE=$vcpkgRoot\scripts\buildsystems\vcpkg.cmake",
    "-DVCPKG_TARGET_TRIPLET=x64-mingw-dynamic"
)
if ((Invoke-Native { cmake @cmakeArgs }) -ne 0) { throw "CMake configure failed" }
if ((Invoke-Native { cmake --build $buildDir }) -ne 0) { throw "Build failed" }

Write-Host ""
Write-Host "Built: $buildDir\bin\goblock.exe" -ForegroundColor Green
Write-Host "Run:   $buildDir\bin\goblock.exe"
