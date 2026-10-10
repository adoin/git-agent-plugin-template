param([Parameter(Mandatory=$true)][string]$Id, [string]$Name, [ValidateSet("github","minimal")][string]$Example = "minimal")
$ErrorActionPreference = 'Stop'
$arguments = @((Join-Path $PSScriptRoot 'plugin.py'), 'init', $Id)
if ($Name) { $arguments += @('--name', $Name) }
$arguments += @("--example", $Example)
& python @arguments
if ($LASTEXITCODE -ne 0) { throw 'Plugin initialization failed' }
