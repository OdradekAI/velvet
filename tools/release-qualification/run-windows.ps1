param(
    [string]$OutputDirectory = "artifacts/release-qualification/windows-deferred",
    [switch]$AllowDeferred
)

if (-not $AllowDeferred) {
    Write-Error "Windows release qualification is intentionally deferred because no Windows environment is available. Re-run with -AllowDeferred to record that result."
    exit 2
}

New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$result = [ordered]@{
    platform = "windows"
    status = "deferred"
    persistence = "deferred"
    acl = "deferred"
    readonly = "deferred"
    removableVolume = "deferred"
    networkVolume = "deferred"
    reason = "No Windows environment is available; cross-target compilation does not qualify Windows filesystem or process behavior."
    evidence = (Resolve-Path $OutputDirectory).Path
}
$result | ConvertTo-Json | Set-Content -Encoding UTF8 (Join-Path $OutputDirectory "result.json")
$result | ConvertTo-Json
