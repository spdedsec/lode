$ErrorActionPreference = "Stop"
$Version = if ($env:LODE_VERSION) { $env:LODE_VERSION } else { "latest" }
$Repo = if ($env:LODE_REPO) { $env:LODE_REPO } else { "spdedsec/lode" }
$Target = "x86_64-pc-windows-msvc"
$Asset = "lode-$Target.zip"
$Base = if ($Version -eq "latest") { "https://github.com/$Repo/releases/latest/download" } else { "https://github.com/$Repo/releases/download/v$Version" }
$Tmp = Join-Path $env:TEMP ("lode-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $Tmp | Out-Null
try {
  $Archive = Join-Path $Tmp $Asset
  Invoke-WebRequest "$Base/$Asset" -OutFile $Archive
  $Sums = Join-Path $Tmp "SHA256SUMS.txt"
  Invoke-WebRequest "$Base/SHA256SUMS.txt" -OutFile $Sums
  $Line = Get-Content $Sums | Where-Object { $_ -match [regex]::Escape($Asset) } | Select-Object -First 1
  if (-not $Line) { throw "No checksum for $Asset" }
  $Expected = ($Line -split '\s+')[0].ToLowerInvariant()
  $Actual = (Get-FileHash $Archive -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($Expected -ne $Actual) { throw "Checksum verification failed" }
  $InstallDir = if ($env:LODE_INSTALL_DIR) { $env:LODE_INSTALL_DIR } else { Join-Path $HOME ".lode\bin" }
  New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
  Expand-Archive $Archive -DestinationPath $Tmp -Force
  Copy-Item (Join-Path $Tmp "lode-$Target\lode.exe") (Join-Path $InstallDir "lode.exe") -Force
  Write-Host "LODE installed to $InstallDir\lode.exe"
} finally { Remove-Item $Tmp -Recurse -Force -ErrorAction SilentlyContinue }
