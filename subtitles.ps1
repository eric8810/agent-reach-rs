# Subtitle batch downloader + metadata extractor
# Usage: powershell -File subtitles.ps1
# Reads pipeline_data.json, downloads subtitles for all videos

$ErrorActionPreference = "Continue"
$exe = ".\target\debug\agent-reach.exe"
$dataFile = "pipeline_data.json"
$subsDir = "subtitles"
$metaFile = "video_metadata.json"

New-Item -ItemType Directory -Force -Path $subsDir | Out-Null

$data = Get-Content $dataFile -Raw -Encoding UTF8 | ConvertFrom-Json
$meta = @{}
if (Test-Path $metaFile) { $meta = Get-Content $metaFile -Raw -Encoding UTF8 | ConvertFrom-Json | ForEach-Object { @{} + $_ } }

$total = $data.Count
$done = 0
$subCount = 0

foreach ($v in $data) {
    $done++
    $vid = $v.video_id
    $subFile = "$subsDir/$vid.txt"
    $metaFile_v = "$subsDir/$vid.json"

    # Skip if already downloaded
    if (Test-Path $subFile) { $subCount++; continue }

    # Get video info for publish date
    if (-not (Test-Path $metaFile_v)) {
        try {
            $cmd = ".`\target\debug\agent-reach.exe youtube info `"$vid`""
            $raw = Invoke-Expression $cmd 2>&1 | Out-String
            $lines = $raw -split "[`r`n]"
            $info = @{video_id=$vid}
            foreach ($line in $lines) {
                if ($line -match '^Title:\s*(.+)') { $info.title = $matches[1].Trim() }
                if ($line -match '^Author:\s*(.+)') { $info.author = $matches[1].Trim() }
                if ($line -match '^Length:\s*(\d+)s') { $info.length_sec = [int]$matches[1] }
                if ($line -match 'Views:\s*(.+)') { $info.views = $matches[1].Trim() }
            }
            $info | ConvertTo-Json | Set-Content $metaFile_v -Encoding UTF8 -Force
            $meta[$vid] = $info
        } catch {}
    }

    # Download subtitles
    try {
        $cmd = ".`\target\debug\agent-reach.exe youtube subtitles `"$vid`""
        $raw = Invoke-Expression $cmd 2>&1 | Out-String
        if ($raw -notmatch "Failed|No caption|not found") {
            $raw | Set-Content $subFile -Encoding UTF8 -Force
            $subCount++
        }
    } catch {}

    if ($done % 10 -eq 0) {
        Write-Host "[$done/$total] Videos processed, $subCount subtitles downloaded" -ForegroundColor DarkGray
    }
}

Write-Host "`nSUBTITLE DOWNLOAD COMPLETE" -ForegroundColor Green
Write-Host "Total videos: $total" -ForegroundColor Green
Write-Host "With subtitles: $subCount" -ForegroundColor Green
