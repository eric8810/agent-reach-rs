# AI Agent YouTube Ecosystem Investigation — Unified Pipeline
# One command: powershell -File run_all.ps1
# Produces: pipeline_data.json + subtitles/ + analyze.ps1 output + INVESTIGATION.md

param([switch]$skip_index, [switch]$skip_subs, [switch]$skip_analysis)

$ErrorActionPreference = "Continue"
$exe = ".\target\debug\agent-reach.exe"

# ── STEP 1: Index ─────────────────────────────────────
if (-not $skip_index) {
    Write-Host "=== STEP 1: INDEX ===" -ForegroundColor Cyan
    . .\pipeline.ps1 2>&1 | Out-Null
}

# ── STEP 2: Subtitles ──────────────────────────────────
if (-not $skip_subs) {
    Write-Host "=== STEP 2: SUBTITLES ===" -ForegroundColor Cyan
    . .\subtitles.ps1 2>&1 | Out-Null
}

# ── STEP 3: Analyze ────────────────────────────────────
if (-not $skip_analysis) {
    Write-Host "=== STEP 3: ANALYZE ===" -ForegroundColor Cyan
    . .\analyze.ps1
}

# ── STEP 4: Report ─────────────────────────────────────
Write-Host "=== STEP 4: REPORT ===" -ForegroundColor Cyan
$data = Get-Content "pipeline_data.json" -Raw -Encoding UTF8 | ConvertFrom-Json
$subs = Get-ChildItem "subtitles\*.txt" -ErrorAction SilentlyContinue | Measure-Object

Write-Host "Videos indexed: $($data.Count)"
Write-Host "Subtitles downloaded: $($subs.Count)"
Write-Host "Channels: $(($data | Where-Object { $_.channel } | Group-Object { $_.channel }).Count)"
Write-Host "`nAll outputs in repo. See INVESTIGATION.md for full report." -ForegroundColor Green
