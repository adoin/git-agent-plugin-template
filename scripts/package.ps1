param([string]$Target, [ValidateSet("github","minimal")][string]$Example = "github")
$ErrorActionPreference = 'Stop'
$arguments = @((Join-Path $PSScriptRoot 'plugin.py'), 'package')
if ($Target) { $arguments += @('--target', $Target) }
$arguments += @("--example", $Example)
& python @arguments
if ($LASTEXITCODE -ne 0) { throw 'Plugin packaging failed' }
