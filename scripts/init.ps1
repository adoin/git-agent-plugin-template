param([Parameter(Mandatory=$true)][string]$Id, [string]$Name)
$ErrorActionPreference = 'Stop'
$arguments = @((Join-Path $PSScriptRoot 'plugin.py'), 'init', $Id)
if ($Name) { $arguments += @('--name', $Name) }
& python @arguments
if ($LASTEXITCODE -ne 0) { throw 'Plugin initialization failed' }
