param([string]$Target)
$ErrorActionPreference = 'Stop'
$arguments = @((Join-Path $PSScriptRoot 'plugin.py'), 'package')
if ($Target) { $arguments += @('--target', $Target) }
& python @arguments
if ($LASTEXITCODE -ne 0) { throw 'Plugin packaging failed' }
