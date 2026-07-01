# AI Agent YouTube Ecosystem Full Index Pipeline
# Usage: powershell -File pipeline.ps1
# Re-runnable: skips already-executed queries

$ErrorActionPreference = "Continue"
$exe = ".\target\debug\agent-reach.exe"
$dataFile = "pipeline_data.json"

# Load existing data if any
$all = @{}
if (Test-Path $dataFile) {
    $existing = Get-Content $dataFile -Raw -Encoding UTF8 | ConvertFrom-Json
    foreach ($v in $existing) { $all[$v.video_id] = $v }
    Write-Host "Loaded $($all.Count) existing videos" -ForegroundColor Green
}

$seenQueries = @{}
foreach ($v in $all.Values) { $seenQueries[$v.query] = $true }

function Search-YT($query, $limit=20) {
    if ($seenQueries.ContainsKey($query)) { return }
    $seenQueries[$query] = $true
    Write-Host "  YT: $query" -ForegroundColor DarkGray
    $cmd = ".`\target\debug\agent-reach.exe youtube search `"$query`" --limit $limit --json"
    try {
        $raw = Invoke-Expression $cmd 2>&1 | Out-String
        $json = $raw | ConvertFrom-Json -ErrorAction SilentlyContinue
        if ($json) {
            foreach ($v in $json) {
                $vid = $v.video_id
                if ($vid -and -not $all.ContainsKey($vid)) {
                    $all[$vid] = @{video_id=$vid; title=$v.title; channel=$v.channel; 
                                   views=$v.views; length=$v.length; published=$v.published;
                                   query=$query; source="youtube"; round=$global:round}
                }
            }
            Write-Host "    +$($json.Count) results, $($all.Count) total" -ForegroundColor DarkGray
        }
    } catch {}
}

function Search-Bili($query, $limit=20) {
    if ($seenQueries.ContainsKey("bili:$query")) { return }
    $seenQueries["bili:$query"] = $true
    Write-Host "  BL: $query" -ForegroundColor DarkGray
    # B站 search via agent-reach bilibili channel - use search method
    # For now, use YouTube as primary source
}

function Save-Data {
    $list = New-Object System.Collections.ArrayList
    foreach ($kv in $all.GetEnumerator()) { $null = $list.Add($kv.Value) }
    $list | ConvertTo-Json -Depth 2 | Set-Content $dataFile -Encoding UTF8 -Force
}

# === ROUND 1: Broad concept scan ===
$global:round = 1
Write-Host "`n=== ROUND 1: Broad Concepts ===" -ForegroundColor Cyan

$broad = @(
    "AI agent", "agentic AI", "autonomous agent", "LLM agent",
    "AI assistant agent", "multi agent system", "agent workflow",
    "agent framework", "agent tool", "agent platform",
    "AI agent tutorial", "AI agent course", "AI agent explained",
    "AI agent build", "AI agent demo", "AI agent review",
    "AI agent vs", "AI agent comparison", "AI agent architecture",
    "AI agent production", "AI agent deployment", "AI agent enterprise"
)
foreach ($q in $broad) { Search-YT $q 20 }
Save-Data

# === ROUND 2: Drill into discovered concepts ===
$global:round = 2
Write-Host "`n=== ROUND 2: Discovered Concepts ===" -ForegroundColor Cyan

# Extract frequent keywords from titles
$wordCounts = @{}
foreach ($v in $all.Values) {
    $words = $v.title.ToLower() -split '[^a-z0-9]' | Where-Object { $_.Length -gt 2 }
    $seen = @{}
    foreach ($w in $words) {
        if (-not $seen.ContainsKey($w)) {
            $seen[$w] = $true
            $wordCounts[$w] = 1 + $wordCounts[$w]
        }
    }
}
$topWords = $wordCounts.GetEnumerator() | Sort-Object Value -Descending | Select-Object -First 80 | ForEach-Object { $_.Key }

# Generate drill-down queries from top concepts
$drill = @()
foreach ($w in @("agentic","autonomous","multi-agent","langgraph","crewai","claude","gemini","copilot","cursor",
                 "openai","rag","mcp","browser","n8n","enterprise","production","deployment","scaling",
                 "monitoring","evaluation","benchmark","safety","security","guardrail","jailbreak",
                 "finance","trading","healthcare","devops","kubernetes","code","coding","software",
                 "memory","planning","reasoning","orchestration","swarm","collective")) {
    if ($topWords -contains $w) {
        $drill += "$w AI agent"
        $drill += "$w agent tutorial"  
        $drill += "$w agent explained"
    }
}
foreach ($q in ($drill | Select-Object -First 50)) { Search-YT $q 20 }
Save-Data

# === ROUND 3: Temporal + Niche ===
$global:round = 3
Write-Host "`n=== ROUND 3: Temporal + Niche ===" -ForegroundColor Cyan

$niche = @(
    "AI agent June 2026", "AI agent May 2026", "AI agent April 2026",
    "AI agent news 2026", "AI agent breakthrough", "AI agent product launch",
    "AI agent startup", "AI agent investment", "AI agent funding",
    "AI agent open source", "AI agent GitHub", "AI agent community",
    "AI agent conference", "AI agent talk", "AI agent keynote",
    "AI agent research paper", "AI agent academic", "AI agent PhD",
    "AI agent failure", "AI agent limitation", "AI agent criticism",
    "AI agent hype", "AI agent reality", "AI agent honest review",
    "AI agent ethics", "AI agent regulation", "AI agent policy",
    "AI agent job", "AI agent career", "AI agent skill"
)
foreach ($q in $niche) { Search-YT $q 20 }
Save-Data

# === ROUND 4: Creator + Platform deep dive ===
$global:round = 4
Write-Host "`n=== ROUND 4: Creator + Platform ===" -ForegroundColor Cyan

# Find top channels
$channelCounts = @{}
foreach ($v in $all.Values) {
    if ($v.channel) { $channelCounts[$v.channel] = 1 + $channelCounts[$v.channel] }
}
$topChannels = $channelCounts.GetEnumerator() | Sort-Object Value -Descending | Select-Object -First 30

foreach ($ch in $topChannels) {
    Search-YT "$($ch.Key) agent" 5
}
Save-Data

Write-Host "`n=== PIPELINE COMPLETE ===" -ForegroundColor Green
Write-Host "Total unique videos: $($all.Count)" -ForegroundColor Green
Write-Host "Total queries: $($seenQueries.Count)" -ForegroundColor Green
