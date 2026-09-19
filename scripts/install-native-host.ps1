# Register the HavenKeys native messaging host with Chrome, Edge, Brave and
# Firefox for the current user (Windows).
#
#   powershell -ExecutionPolicy Bypass -File scripts\install-native-host.ps1 [-HostPath path\to\havenkeys-native-host.exe]
#   powershell -ExecutionPolicy Bypass -File scripts\install-native-host.ps1 -Uninstall
#
# Build the host first: cargo build --release -p havenkeys-native-host
param(
  [string]$HostPath = "target\release\havenkeys-native-host.exe",
  [switch]$Uninstall
)
$ErrorActionPreference = "Stop"

$Name = "com.havenkeys.bridge"
$ChromeOrigin = "chrome-extension://olbclkanfbmilnmfhoojgcnpgdmilfmf/"
$FirefoxId = "havenkeys@havenkeys.app"
$Dir = Join-Path $env:LOCALAPPDATA "HavenKeys"
$ChromeManifest = Join-Path $Dir "$Name.chrome.json"
$FirefoxManifest = Join-Path $Dir "$Name.firefox.json"

$ChromiumKeys = @(
  "HKCU:\Software\Google\Chrome\NativeMessagingHosts\$Name",
  "HKCU:\Software\Microsoft\Edge\NativeMessagingHosts\$Name",
  "HKCU:\Software\BraveSoftware\Brave-Browser\NativeMessagingHosts\$Name",
  "HKCU:\Software\Chromium\NativeMessagingHosts\$Name"
)
$FirefoxKey = "HKCU:\Software\Mozilla\NativeMessagingHosts\$Name"

if ($Uninstall) {
  foreach ($k in $ChromiumKeys + $FirefoxKey) {
    if (Test-Path $k) { Remove-Item -Path $k -Force }
  }
  Remove-Item -Path $ChromeManifest, $FirefoxManifest -Force -ErrorAction SilentlyContinue
  Write-Host "Removed the HavenKeys native host registration."
  exit 0
}

if (-not (Test-Path $HostPath -PathType Leaf)) {
  Write-Error "Native host not found at $HostPath. Build it with: cargo build --release -p havenkeys-native-host"
}
$HostFull = (Resolve-Path $HostPath).Path
New-Item -ItemType Directory -Force -Path $Dir | Out-Null

# ConvertTo-Json escapes the path; files are written as UTF-8 without a BOM.
function Write-Manifest([string]$Path, [string]$Key, [string]$Value) {
  $json = [ordered]@{
    name = $Name; description = "HavenKeys desktop bridge"; path = $HostFull; type = "stdio"
    $Key = @($Value)
  } | ConvertTo-Json
  [System.IO.File]::WriteAllText($Path, $json, (New-Object System.Text.UTF8Encoding $false))
}
Write-Manifest $ChromeManifest "allowed_origins" $ChromeOrigin
Write-Manifest $FirefoxManifest "allowed_extensions" $FirefoxId

foreach ($k in $ChromiumKeys) {
  New-Item -Path $k -Force | Out-Null
  Set-Item -Path $k -Value $ChromeManifest
}
New-Item -Path $FirefoxKey -Force | Out-Null
Set-Item -Path $FirefoxKey -Value $FirefoxManifest

Write-Host "Registered $HostFull for Chrome, Edge, Brave, Chromium and Firefox."
