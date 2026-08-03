# Build a full wheel package including the default web app.
# Run from repo root: pwsh -File scripts/wheel_build.ps1

$ErrorActionPreference = "Stop"
$RepoRoot = (Get-Item $PSScriptRoot).Parent.FullName
Set-Location $RepoRoot

$AppDir = Join-Path $RepoRoot "app"
$WebDest = Join-Path $RepoRoot "src\qwenpaw\console"

Write-Host "[wheel_build] Building default web app..."
Push-Location $AppDir
try {
  npm ci
  if ($LASTEXITCODE -ne 0) { throw "npm ci failed with exit code $LASTEXITCODE" }
  npm run build
  if ($LASTEXITCODE -ne 0) { throw "npm run build failed with exit code $LASTEXITCODE" }
} finally {
  Pop-Location
}

Write-Host "[wheel_build] Copying app/dist/* -> src/qwenpaw/console/..."
if (Test-Path $WebDest) {
  Remove-Item -Path (Join-Path $WebDest "*") -Recurse -Force -ErrorAction SilentlyContinue
} else {
  New-Item -ItemType Directory -Force -Path $WebDest | Out-Null
}
$AppDist = Join-Path $AppDir "dist"
Copy-Item -Path (Join-Path $AppDist "*") -Destination $WebDest -Recurse -Force

Write-Host "[wheel_build] Bundling website docs into package..."
$DocsSrc = Join-Path $RepoRoot "website\public\docs"
$DocsDest = Join-Path $RepoRoot "src\qwenpaw\docs"
if (Test-Path $DocsDest) { Remove-Item -Recurse -Force $DocsDest }
New-Item -ItemType Directory -Force -Path $DocsDest | Out-Null
Copy-Item -Path (Join-Path $DocsSrc "*.md") -Destination $DocsDest -Force

Write-Host "[wheel_build] Building wheel + sdist..."
python -m pip install --quiet build
$DistDir = Join-Path $RepoRoot "dist"
if (Test-Path $DistDir) {
  Remove-Item -Path (Join-Path $DistDir "*") -Force -ErrorAction SilentlyContinue
}
python -m build --outdir dist .
if ($LASTEXITCODE -ne 0) { throw "python -m build failed with exit code $LASTEXITCODE" }

Write-Host "[wheel_build] Done. Wheel(s) in: $RepoRoot\dist\"
