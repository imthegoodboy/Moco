[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$runtime = Join-Path $projectRoot 'src-tauri\resources\runtime\llama-server.exe'
$model = Join-Path $projectRoot 'src-tauri\resources\models\LFM2.5-230M-Q4_K_M.gguf'
$port = 39291

if (-not (Test-Path -LiteralPath $runtime)) { throw "Missing runtime: $runtime" }
if (-not (Test-Path -LiteralPath $model)) { throw "Missing model: $model" }

$arguments = @('-m', $model, '--host', '127.0.0.1', '--port', $port, '--ctx-size', '2048', '--threads', '4', '--n-gpu-layers', '0', '--no-webui')
$process = Start-Process -FilePath $runtime -ArgumentList $arguments -PassThru -WindowStyle Hidden
try {
    $ready = $false
    for ($attempt = 0; $attempt -lt 120; $attempt++) {
        if ($process.HasExited) { throw "llama.cpp exited before the model became ready (code $($process.ExitCode))." }
        try {
            $health = Invoke-RestMethod -Uri "http://127.0.0.1:$port/health" -TimeoutSec 1
            if ($health.status -eq 'ok') { $ready = $true; break }
        } catch {}
        Start-Sleep -Milliseconds 250
    }
    if (-not $ready) { throw 'The local runtime did not become healthy within 30 seconds.' }

    $body = @{
        model = 'local-model'
        stream = $false
        max_tokens = 32
        temperature = 0
        messages = @(
            @{ role = 'system'; content = 'Reply with exactly: MOCO OFFLINE READY' },
            @{ role = 'user'; content = 'Status check' }
        )
    } | ConvertTo-Json -Depth 6
    $response = Invoke-RestMethod -Method Post -Uri "http://127.0.0.1:$port/v1/chat/completions" -ContentType 'application/json' -Body $body -TimeoutSec 60
    $content = $response.choices[0].message.content
    if ([string]::IsNullOrWhiteSpace($content)) { throw 'The model returned an empty response.' }
    Write-Host "Offline inference verified: $content"
} finally {
    if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force }
}
