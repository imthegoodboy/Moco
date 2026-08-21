[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$runtimeDirectory = Join-Path $projectRoot 'src-tauri\resources\runtime'
$modelDirectory = Join-Path $projectRoot 'src-tauri\resources\models'
$licenseDirectory = Join-Path $projectRoot 'third_party\licenses'
$temporaryRoot = Join-Path ([IO.Path]::GetFullPath($env:TEMP)) ("moco-assets-" + [Guid]::NewGuid().ToString('N'))

$llamaTag = 'b10502'
$llamaArchiveName = 'llama-b10502-bin-win-cpu-x64.zip'
$llamaUrl = "https://github.com/ggml-org/llama.cpp/releases/download/$llamaTag/$llamaArchiveName"
$llamaSha256 = 'bf009ffc8a7c0ce8631122668281f1df39c16856efb9270c3de805cf6d4b211f'

$modelRevision = 'fb5e743241d08c98626e04c13828feffae4acdfb'
$modelName = 'LFM2.5-230M-Q4_K_M.gguf'
$modelRepository = 'LiquidAI/LFM2.5-230M-GGUF'
$modelUrl = "https://huggingface.co/$modelRepository/resolve/$modelRevision/${modelName}?download=true"
$modelSha256 = '7bbd90384d3deffe4c646ec9643b212802d32d4ce417c90a1ec9282100650062'

function Get-VerifiedAsset {
    param(
        [Parameter(Mandatory)] [string] $Uri,
        [Parameter(Mandatory)] [string] $Destination,
        [Parameter(Mandatory)] [string] $ExpectedSha256,
        [long] $ExpectedBytes = 0,
        [string] $ChunkDirectory = ''
    )

    if ($ExpectedBytes -gt 100MB) {
        Get-ParallelVerifiedAsset -Uri $Uri -Destination $Destination -ExpectedSha256 $ExpectedSha256 -ExpectedBytes $ExpectedBytes -ChunkDirectory $ChunkDirectory
        return
    }

    for ($attempt = 1; $attempt -le 3; $attempt++) {
        if ((Test-Path -LiteralPath $Destination) -and ((Get-Sha256 -Path $Destination) -eq $ExpectedSha256.ToLowerInvariant())) {
            return
        }
        $curlArguments = @('--fail', '--location', '--silent', '--show-error', '--retry', '3', '--retry-delay', '2', '--continue-at', '-', '--output', $Destination, $Uri)
        & curl.exe @curlArguments
        if ($LASTEXITCODE -ne 0) {
            if ($attempt -eq 3) { throw "Download failed for $Destination." }
            continue
        }
        $actual = Get-Sha256 -Path $Destination
        if ($actual -eq $ExpectedSha256.ToLowerInvariant()) {
            return
        }
        if ($attempt -lt 3) {
            Write-Warning "The download was incomplete or changed (attempt $attempt of 3). Retrying..."
            Remove-Item -LiteralPath $Destination -Force
        }
    }
    if (Test-Path -LiteralPath $Destination) {
        Remove-Item -LiteralPath $Destination -Force
    }
    throw "Integrity check failed for $Destination after 3 attempts. Expected $ExpectedSha256 but received $actual."
}

function Get-ParallelVerifiedAsset {
    param(
        [Parameter(Mandatory)] [string] $Uri,
        [Parameter(Mandatory)] [string] $Destination,
        [Parameter(Mandatory)] [string] $ExpectedSha256,
        [Parameter(Mandatory)] [long] $ExpectedBytes,
        [Parameter(Mandatory)] [string] $ChunkDirectory
    )

    if ((Test-Path -LiteralPath $Destination) -and ((Get-Sha256 -Path $Destination) -eq $ExpectedSha256.ToLowerInvariant())) {
        return
    }

    if ([string]::IsNullOrWhiteSpace($ChunkDirectory)) {
        throw 'A temporary chunk directory is required for parallel downloads.'
    }

    New-Item -ItemType Directory -Path $ChunkDirectory -Force | Out-Null
    $chunkCount = 8
    $chunkSize = [long][Math]::Ceiling($ExpectedBytes / $chunkCount)
    $chunks = @()

    for ($index = 0; $index -lt $chunkCount; $index++) {
        $start = [long]$index * $chunkSize
        $end = [Math]::Min($ExpectedBytes - 1, $start + $chunkSize - 1)
        $path = Join-Path $ChunkDirectory ("part-{0:D2}" -f $index)
        $chunks += [PSCustomObject]@{
            Start = $start
            End = $end
            Path = $path
            Length = ($end - $start + 1)
        }
    }

    for ($attempt = 1; $attempt -le 3; $attempt++) {
        $jobs = @()
        foreach ($chunk in $chunks) {
            if ((Test-Path -LiteralPath $chunk.Path) -and ((Get-Item -LiteralPath $chunk.Path).Length -eq $chunk.Length)) {
                continue
            }
            if (Test-Path -LiteralPath $chunk.Path) {
                Remove-Item -LiteralPath $chunk.Path -Force
            }
            $jobs += Start-Job -ScriptBlock {
                param($DownloadUri, $OutputPath, $RangeStart, $RangeEnd)
                & curl.exe --fail --location --silent --show-error --retry 3 --retry-delay 2 --range "$RangeStart-$RangeEnd" --output $OutputPath $DownloadUri
                if ($LASTEXITCODE -ne 0) {
                    throw "Chunk download failed with exit code $LASTEXITCODE."
                }
            } -ArgumentList $Uri,$chunk.Path,$chunk.Start,$chunk.End
        }

        if ($jobs.Count -gt 0) {
            Write-Host "Downloading $($jobs.Count) model chunks in parallel (attempt $attempt of 3)..."
            $jobs | Wait-Job | Out-Null
            $failedJobs = @($jobs | Where-Object State -ne 'Completed')
            $jobs | Receive-Job -ErrorAction SilentlyContinue
            $jobs | Remove-Job -Force
            if ($failedJobs.Count -gt 0) {
                if ($attempt -eq 3) { throw 'One or more model chunks failed to download.' }
                continue
            }
        }

        $invalidChunks = @($chunks | Where-Object { -not (Test-Path -LiteralPath $_.Path) -or (Get-Item -LiteralPath $_.Path).Length -ne $_.Length })
        if ($invalidChunks.Count -gt 0) {
            if ($attempt -eq 3) { throw 'One or more model chunks have an unexpected size.' }
            continue
        }

        if (Test-Path -LiteralPath $Destination) {
            Remove-Item -LiteralPath $Destination -Force
        }
        $output = [IO.File]::Create($Destination)
        try {
            foreach ($chunk in $chunks) {
                $input = [IO.File]::OpenRead($chunk.Path)
                try {
                    $input.CopyTo($output)
                } finally {
                    $input.Dispose()
                }
            }
        } finally {
            $output.Dispose()
        }

        $actual = Get-Sha256 -Path $Destination
        if ($actual -eq $ExpectedSha256.ToLowerInvariant()) {
            return
        }

        Remove-Item -LiteralPath $Destination -Force
        $chunks | ForEach-Object { Remove-Item -LiteralPath $_.Path -Force -ErrorAction SilentlyContinue }
        if ($attempt -eq 3) {
            throw "Integrity check failed for $Destination. Expected $ExpectedSha256 but received $actual."
        }
    }
}

function Get-Sha256 {
    param([Parameter(Mandatory)] [string] $Path)
    $stream = [IO.File]::OpenRead($Path)
    try {
        $hasher = [Security.Cryptography.SHA256]::Create()
        try {
            return ([BitConverter]::ToString($hasher.ComputeHash($stream))).Replace('-', '').ToLowerInvariant()
        } finally {
            $hasher.Dispose()
        }
    } finally {
        $stream.Dispose()
    }
}

New-Item -ItemType Directory -Path $temporaryRoot,$runtimeDirectory,$modelDirectory,$licenseDirectory -Force | Out-Null
try {
    $archivePath = Join-Path $temporaryRoot $llamaArchiveName
    Write-Host "Downloading the pinned llama.cpp Windows runtime ($llamaTag)..."
    Get-VerifiedAsset -Uri $llamaUrl -Destination $archivePath -ExpectedSha256 $llamaSha256
    $expanded = Join-Path $temporaryRoot 'llama'
    Expand-Archive -LiteralPath $archivePath -DestinationPath $expanded -Force
    Get-ChildItem -LiteralPath $expanded -File | Copy-Item -Destination $runtimeDirectory -Force

    $modelPath = Join-Path $modelDirectory $modelName
    if ((Test-Path -LiteralPath $modelPath) -and ((Get-Sha256 -Path $modelPath) -eq $modelSha256)) {
        Write-Host 'The built-in model is already present and verified.'
    } else {
        Write-Host 'Downloading the pinned LFM2.5-230M Q4_K_M model (about 153 MB)...'
        Get-VerifiedAsset -Uri $modelUrl -Destination $modelPath -ExpectedSha256 $modelSha256 -ExpectedBytes 153406304 -ChunkDirectory (Join-Path $temporaryRoot 'model-chunks')
    }

    Invoke-WebRequest -Uri 'https://raw.githubusercontent.com/ggml-org/llama.cpp/master/LICENSE' -OutFile (Join-Path $licenseDirectory 'llama.cpp-LICENSE.txt') -UseBasicParsing
    Invoke-WebRequest -Uri "https://huggingface.co/$modelRepository/resolve/$modelRevision/LICENSE" -OutFile (Join-Path $licenseDirectory 'LiquidAI-LFM-LICENSE.txt') -UseBasicParsing
    Write-Host 'Runtime assets are ready.'
} finally {
    $resolvedTemporary = [IO.Path]::GetFullPath($temporaryRoot)
    $resolvedTempBase = [IO.Path]::GetFullPath($env:TEMP)
    if ($resolvedTemporary.StartsWith($resolvedTempBase, [StringComparison]::OrdinalIgnoreCase) -and (Test-Path -LiteralPath $resolvedTemporary)) {
        Remove-Item -LiteralPath $resolvedTemporary -Recurse -Force
    }
}
