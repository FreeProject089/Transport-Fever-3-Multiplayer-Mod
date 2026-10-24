$ErrorActionPreference = 'Stop'
$picker = Join-Path $PSScriptRoot 'select-game-archive.ps1'
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$testRoot = Join-Path $tempRoot ("tpf3mp-archive-selector-{0}-{1}" -f $PID, [Guid]::NewGuid().ToString('N'))
$testRoot = [IO.Path]::GetFullPath($testRoot)
if (-not $testRoot.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase)) {
  throw 'Test directory escaped the temporary directory'
}
try {
  $old = New-Item -ItemType Directory -Path (Join-Path $testRoot 'old') -Force
  $updated = New-Item -ItemType Directory -Path (Join-Path $testRoot 'updated') -Force
  $b40423 = New-Item -ItemType Directory -Path (Join-Path $testRoot '40423') -Force
  $b40425 = New-Item -ItemType Directory -Path (Join-Path $testRoot '40425') -Force

  $chosen = & $picker -SelectedBuild tf3_build40408_steam_windows -ReleaseArchive $old.FullName -UpdatedArchive $updated.FullName -Archive40423 $b40423.FullName -Archive40425 $b40425.FullName
  if ($chosen -ne $old.FullName) { throw '40408 must use its own archive' }
  $chosen = & $picker -SelectedBuild tf3_build40420_steam_windows -ReleaseArchive $old.FullName -UpdatedArchive $updated.FullName -Archive40423 $b40423.FullName -Archive40425 $b40425.FullName
  if ($chosen -ne $updated.FullName) { throw '40420 must use its own archive' }
  $chosen = & $picker -SelectedBuild tf3_build40423_steam_windows -ReleaseArchive $old.FullName -UpdatedArchive $updated.FullName -Archive40423 $b40423.FullName -Archive40425 $b40425.FullName
  if ($chosen -ne $b40423.FullName) { throw '40423 must use its own archive' }
  $chosen = & $picker -SelectedBuild tf3_build40425_steam_windows -ReleaseArchive $old.FullName -UpdatedArchive $updated.FullName -Archive40423 $b40423.FullName -Archive40425 $b40425.FullName
  if ($chosen -ne $b40425.FullName) { throw '40425 must use its own archive' }

  foreach ($scenario in @(
    @{ Build = 'tf3_build40420_steam_windows'; New = ''; New40423 = $b40423.FullName; New40425 = $b40425.FullName },
    @{ Build = 'tf3_build99999_steam_windows'; New = $updated.FullName; New40423 = $b40423.FullName; New40425 = $b40425.FullName },
    @{ Build = 'tf3_build40420_steam_windows'; New = (Join-Path $testRoot 'missing'); New40423 = $b40423.FullName; New40425 = $b40425.FullName },
    @{ Build = 'tf3_build40423_steam_windows'; New = $updated.FullName; New40423 = ''; New40425 = $b40425.FullName },
    @{ Build = 'tf3_build40423_steam_windows'; New = $updated.FullName; New40423 = (Join-Path $testRoot 'missing'); New40425 = $b40425.FullName },
    @{ Build = 'tf3_build40425_steam_windows'; New = $updated.FullName; New40423 = $b40423.FullName; New40425 = '' },
    @{ Build = 'tf3_build40425_steam_windows'; New = $updated.FullName; New40423 = $b40423.FullName; New40425 = (Join-Path $testRoot 'missing') }
  )) {
    $refused = $false
    try {
      & $picker -SelectedBuild $scenario.Build -ReleaseArchive $old.FullName -UpdatedArchive $scenario.New -Archive40423 $scenario.New40423 -Archive40425 $scenario.New40425 | Out-Null
    } catch {
      $refused = $true
    }
    if (-not $refused) { throw "Unsupported or missing archive was accepted: $($scenario.Build)" }
  }
  'archive selection: each build its own archive; missing and unknown builds refused'
} finally {
  if (Test-Path -LiteralPath $testRoot) {
    Remove-Item -LiteralPath $testRoot -Recurse -Force
  }
}
