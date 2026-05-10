Set-StrictMode -Version Latest

function Get-GitChangedPaths {
    $tracked = @(& git diff --name-only HEAD -- | Where-Object { $_ })
    $untracked = @(& git ls-files --others --exclude-standard | Where-Object { $_ })
    return @($tracked + $untracked | Sort-Object -Unique)
}

function Get-PathSet {
    param([string[]]$Paths)

    $set = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
    foreach ($path in $Paths) {
        [void]$set.Add($path)
    }
    return ,$set
}

function Get-PathSignature {
    param([string]$Path)

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        return "missing"
    }

    $hash = @(& git hash-object -- $Path 2>$null | Select-Object -First 1)
    if ($LASTEXITCODE -eq 0 -and $hash.Count -gt 0 -and $hash[0]) {
        return "blob:$($hash[0])"
    }

    $item = Get-Item -LiteralPath $Path
    return "file:$($item.Length):$($item.LastWriteTimeUtc.Ticks)"
}

function Get-PathSignatureMap {
    param([string[]]$Paths)

    $map = @{}
    foreach ($path in $Paths) {
        $map[$path] = Get-PathSignature -Path $path
    }
    return $map
}

function Get-BatchChangedPaths {
    param(
        [string[]]$CurrentChanged,
        [System.Collections.Generic.HashSet[string]]$BaselineDirty,
        [hashtable]$BaselineSignatures,
        [bool]$IncludeBaselineDirtyChanges
    )

    $paths = New-Object System.Collections.Generic.List[string]
    foreach ($path in $CurrentChanged) {
        if (-not $BaselineDirty.Contains($path)) {
            $paths.Add($path)
            continue
        }

        if (-not $IncludeBaselineDirtyChanges) {
            continue
        }

        $oldSignature = if ($BaselineSignatures.ContainsKey($path)) { $BaselineSignatures[$path] } else { "" }
        $newSignature = Get-PathSignature -Path $path
        if ($newSignature -ne $oldSignature) {
            $paths.Add($path)
        }
    }

    return @($paths | Sort-Object -Unique)
}
