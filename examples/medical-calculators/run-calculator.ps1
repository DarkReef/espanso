param(
    [Parameter(Mandatory = $true, Position = 0)]
    [ValidatePattern('^[a-z0-9][a-z0-9-]*$')]
    [string]$CalculatorId
)

$ErrorActionPreference = 'Stop'
$modulePath = Join-Path $PSScriptRoot "modules\$CalculatorId.ps1"

if (-not (Test-Path -LiteralPath $modulePath -PathType Leaf)) {
    Write-Error "Calculator module not found: $CalculatorId"
    exit 2
}

try {
    & $modulePath
    if (-not $?) {
        exit 3
    }
}
catch {
    Write-Error $_.Exception.Message
    exit 4
}
