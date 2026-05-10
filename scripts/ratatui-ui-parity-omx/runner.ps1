Set-StrictMode -Version Latest

function Convert-ToExitCode {
    param([AllowNull()][object]$Value)

    if ($null -eq $Value) {
        return 0
    }

    $items = @($Value)
    for ($i = $items.Count - 1; $i -ge 0; $i--) {
        $candidate = $items[$i]
        if ($null -eq $candidate) {
            continue
        }
        if ($candidate -is [int]) {
            return [int]$candidate
        }

        $parsed = 0
        if ([int]::TryParse(([string]$candidate).Trim(), [ref]$parsed)) {
            return $parsed
        }
    }

    $sample = ($items | Select-Object -First 3 | ForEach-Object { [string]$_ }) -join " | "
    throw "Runner exit code did not contain an integer. Received $($items.Count) object(s): $sample"
}

function Invoke-RunnerProcess {
    param([string[]]$ArgumentList)

    & powershell @ArgumentList | Out-Host
    return (Convert-ToExitCode -Value $LASTEXITCODE)
}
