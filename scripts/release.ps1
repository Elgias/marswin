<#
.SYNOPSIS
  Builds a signed MarsWin release into .\release: installer, portable exe and latest.json.

.DESCRIPTION
  Bump the version in package.json, src-tauri/Cargo.toml and src-tauri/tauri.conf.json first.
  Installed copies of MarsWin read latest.json from the newest GitHub release and update
  themselves, so latest.json must be attached to every release.

  The updater key lives outside the repo (default: ~\.tauri\marswin.key). Keep a backup:
  without it, installed copies can never update again.

.PARAMETER Notes
  Markdown file with the release notes (English). Goes into latest.json and, with -Publish,
  into the GitHub release.

.PARAMETER Publish
  Also creates the GitHub release vX.Y.Z with the three files (needs gh).

.EXAMPLE
  .\scripts\release.ps1 -Notes notes.md
  .\scripts\release.ps1 -Notes notes.md -Publish
#>
param(
  [string]$Notes,
  [switch]$Publish,
  [string]$Key = "$HOME\.tauri\marswin.key"
)
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root

$repo = "Elgias/marswin"
$version = (Get-Content src-tauri\tauri.conf.json -Raw | ConvertFrom-Json).version
$tag = "v$version"
if (-not (Test-Path $Key)) { throw "Updater key not found: $Key" }
if ($Notes -and -not (Test-Path $Notes)) { throw "Notes file not found: $Notes" }

Write-Host "Building MarsWin $version..."
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $Key -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""
npx tauri build
if ($LASTEXITCODE -ne 0) { throw "Build failed" }
Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY, Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD

$bundle = "src-tauri\target\release\bundle\nsis"
$setupName = "MarsWin_${version}_x64-setup.exe"
$setup = Join-Path $bundle $setupName
$sig = "$setup.sig"
if (-not (Test-Path $sig)) { throw "No signature next to the installer: $sig" }

$out = Join-Path $root "release"
New-Item -ItemType Directory -Force $out | Out-Null
Get-ChildItem $out -File | Remove-Item
Copy-Item $setup $out
Copy-Item "src-tauri\target\release\marswin.exe" (Join-Path $out "MarsWin_${version}_x64_portable.exe")

$latest = [ordered]@{
  version   = $version
  notes     = if ($Notes) { (Get-Content $Notes -Raw).Trim() } else { "" }
  pub_date  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
  platforms = [ordered]@{
    "windows-x86_64" = [ordered]@{
      signature = (Get-Content $sig -Raw).Trim()
      url       = "https://github.com/$repo/releases/download/$tag/$setupName"
    }
  }
}
$latest | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $out "latest.json") -Encoding utf8NoBOM

Write-Host ""
Write-Host "Ready in ${out}:"
Get-ChildItem $out | ForEach-Object { Write-Host "  $($_.Name)" }

if ($Publish) {
  $files = Get-ChildItem $out -File | ForEach-Object { $_.FullName }
  $noteArgs = if ($Notes) { @("--notes-file", $Notes) } else { @("--notes", "") }
  gh release create $tag --repo $repo --title "MarsWin $version" @noteArgs @files
  if ($LASTEXITCODE -ne 0) { throw "gh release create failed" }
} else {
  Write-Host ""
  Write-Host "Publish: create release $tag on GitHub and attach all three files (latest.json too)."
}
