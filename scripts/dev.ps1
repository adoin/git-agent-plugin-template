param([string]$HostExecutable, [int]$SmokeTestSeconds = 0)
$ErrorActionPreference = 'Stop'
$pluginRoot = Split-Path -Parent $PSScriptRoot
$arguments = @((Join-Path $PSScriptRoot 'plugin.py'), 'dev')
if ($HostExecutable) { $arguments += @('--host', $HostExecutable) }
if ($SmokeTestSeconds -gt 0) { $arguments += @('--smoke', "$SmokeTestSeconds") }
Push-Location $pluginRoot
try { & python @arguments; if ($LASTEXITCODE -ne 0) { throw 'Plugin dev failed' } }
finally { Pop-Location }
