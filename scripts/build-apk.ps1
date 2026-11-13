param(
    [Parameter(Mandatory=$true)][string]$Workspace,
    [string]$Sdk = 'C:\Android\sdk',
    [string]$NativeTarget,
    [Parameter(Mandatory=$true)][string]$Keystore,
    [string]$Output = 'gridcraft-0.5.0-arm64-v8a-files-ru3.apk',
    [switch]$RunTests,
    [ValidateRange(1,64)][int]$BuildJobs = 2
)
$ErrorActionPreference = 'Stop'
$portRoot = Split-Path $PSScriptRoot -Parent
$Workspace = (Resolve-Path -LiteralPath $Workspace).Path
if (-not $NativeTarget) { $NativeTarget = Join-Path $Workspace 'target' }
$NativeTarget = [System.IO.Path]::GetFullPath($NativeTarget)
$env:CARGO_TARGET_DIR = $NativeTarget
$env:CARGO_BUILD_JOBS = "$BuildJobs"
if (-not (Test-Path -LiteralPath "$Workspace\crates\engine\Cargo.toml")) {
    throw 'Workspace must be the assembled, patched upstream GridCraft workspace.'
}
# Apply the additional interface translation once; accept an already-patched workspace.
$localePatch = Join-Path $portRoot 'patches\0002-russian-dialogs.patch'
$ErrorActionPreference = 'Continue'
& git -C $Workspace apply --check $localePatch 2>$null
$canApplyLocale = $LASTEXITCODE -eq 0
$ErrorActionPreference = 'Stop'
if ($canApplyLocale) {
    & git -C $Workspace apply $localePatch
    if ($LASTEXITCODE -ne 0) { throw 'Interface translation patch failed' }
} else {
    & git -C $Workspace apply --reverse --check $localePatch 2>$null
    if ($LASTEXITCODE -ne 0) { throw 'Workspace does not match the interface translation patch' }
}
$env:ANDROID_HOME = $Sdk
$env:ANDROID_NDK_HOME = "$Sdk\ndk\27.2.12479018"
$env:ANDROID_NDK_ROOT = $env:ANDROID_NDK_HOME
$env:JAVA_HOME = Split-Path (Split-Path (Get-Command javac.exe).Source -Parent) -Parent
$ndkBin = "$env:ANDROID_NDK_HOME\toolchains\llvm\prebuilt\windows-x86_64\bin"
$env:PATH = "$ndkBin;$env:JAVA_HOME\bin;$env:PATH"
$env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = "$ndkBin\aarch64-linux-android26-clang.cmd"
$env:CC_aarch64_linux_android = $env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER
$env:CXX_aarch64_linux_android = "$ndkBin\aarch64-linux-android26-clang++.cmd"
$env:AR_aarch64_linux_android = "$ndkBin\llvm-ar.exe"
$env:CARGO_APK_RELEASE_KEYSTORE = $Keystore
$env:CARGO_APK_RELEASE_KEYSTORE_PASSWORD = 'android'

# The assembled workspace is the source of truth for the build; the port repository is the source
# of truth for the crate. Copy it in, so the two can never disagree by accident.
Copy-Item -LiteralPath "$portRoot\apps\gridcraft-android\Cargo.toml" -Destination "$Workspace\apps\gridcraft-android\Cargo.toml" -Force
Copy-Item -Path "$portRoot\apps\gridcraft-android\src\*.rs" -Destination "$Workspace\apps\gridcraft-android\src" -Force
# Replace the whole folders: a merge into an existing folder can leave stale files behind.
foreach ($folder in @('java', 'res')) {
    $destination = [System.IO.Path]::GetFullPath((Join-Path $Workspace "apps\gridcraft-android\$folder"))
    if (-not $destination.StartsWith($Workspace.TrimEnd('\') + '\', [System.StringComparison]::OrdinalIgnoreCase)) {
        throw 'Crate directory must stay inside Workspace'
    }
    if (Test-Path -LiteralPath $destination) { Remove-Item -LiteralPath $destination -Recurse -Force }
    Copy-Item -LiteralPath "$portRoot\apps\gridcraft-android\$folder" -Destination "$Workspace\apps\gridcraft-android" -Recurse -Force
}
Push-Location $Workspace
try {
    if ($RunTests) {
        & cargo test --lib -p gridcraft-android -p gridcraft-ui-egui
        if ($LASTEXITCODE -ne 0) { throw 'Android shell tests failed' }
    }
    & cargo apk build --release -p gridcraft-android
    if ($LASTEXITCODE -ne 0) { throw 'Rust APK build failed' }
} finally { Pop-Location }

# cargo-apk packages native code only. Compile the Activity and rebuild the resource package with
# its DEX before alignment/signing. No signed APK is edited in place.
$stage = "$portRoot\target\android-bridge"
New-Item -ItemType Directory -Force -Path "$stage\classes", "$stage\dex" | Out-Null
$androidJar = "$Sdk\platforms\android-35\android.jar"
& javac --release 8 -encoding UTF-8 -classpath $androidJar -d "$stage\classes" "$portRoot\apps\gridcraft-android\java\ai\storyteller\gridcraft\GridcraftActivity.java"
if ($LASTEXITCODE -ne 0) { throw 'Activity compilation failed' }
$classFiles = @(Get-ChildItem "$stage\classes" -Recurse -Filter '*.class' | ForEach-Object FullName)
& "$Sdk\build-tools\35.0.0\d8.bat" --min-api 26 --lib $androidJar --output "$stage\dex" @classFiles
if ($LASTEXITCODE -ne 0) { throw 'DEX compilation failed' }
& python "$PSScriptRoot\package-bridge.py" --workspace $Workspace --native-target $NativeTarget --port $portRoot --stage $stage --sdk $Sdk
if ($LASTEXITCODE -ne 0) { throw 'APK packaging failed' }
& "$Sdk\build-tools\35.0.0\zipalign.exe" -f -P 16 4 "$stage\unsigned.apk" "$stage\aligned.apk"
if ($LASTEXITCODE -ne 0) { throw 'APK alignment failed' }
$outputPath = Join-Path $portRoot $Output
& "$Sdk\build-tools\35.0.0\apksigner.bat" sign --ks $Keystore --ks-pass pass:android --out $outputPath "$stage\aligned.apk"
if ($LASTEXITCODE -ne 0) { throw 'APK signing failed' }
& "$Sdk\build-tools\35.0.0\apksigner.bat" verify --verbose $outputPath
if ($LASTEXITCODE -ne 0) { throw 'APK signature verification failed' }
& "$Sdk\build-tools\35.0.0\aapt.exe" dump badging $outputPath
if ($LASTEXITCODE -ne 0) { throw 'APK manifest verification failed' }
Get-FileHash -Algorithm SHA256 -LiteralPath $outputPath
