$exe = ".\target\debug\agent-reach.exe"
$data = Get-Content "pipeline_data.json" -Raw -Encoding UTF8 | ConvertFrom-Json
$total = $data.Count

Write-Host "=== TOPIC CLASSIFICATION ===" -ForegroundColor Cyan
$topics = @{}
$patterns = @(
    @{k="Claude/Anthropic";p="claude"}, @{k="Cursor";p="cursor"}, @{k="Gemini/Google";p="gemini"},
    @{k="Copilot/Microsoft";p="copilot|microsoft"}, @{k="OpenAI";p="openai|chatgpt"},
    @{k="LangGraph";p="langgraph"}, @{k="CrewAI";p="crewai"}, @{k="n8n";p="\bn8n\b"},
    @{k="MCP";p="\bmcp\b"}, @{k="browser-use";p="browser.use|browseruse"},
    @{k="Tutorial/Course";p="tutorial|course|learn|beginner|guide|training"},
    @{k="Production/Deploy";p="production|deploy|scaling"},
    @{k="Multi-Agent";p="multi.agent|multiagent|orchestrat"},
    @{k="Memory";p="\bmemory\b"}, @{k="Planning";p="planning|reasoning"},
    @{k="Evaluation";p="evaluat|benchmark|testing"},
    @{k="Safety";p="safety|security|guardrail|jailbreak"},
    @{k="Finance";p="finance|trading|stock|wealth"},
    @{k="Healthcare";p="health|medical|clinical"},
    @{k="DevOps";p="devops|cloud|kubernetes|infrastructure"},
    @{k="RAG";p="\brag\b|retrieval|vector"},
    @{k="Enterprise";p="enterprise|business|company|roi"},
    @{k="Open Source";p="open.source|github"}, @{k="Agentic";p="agentic|autonomous"}
)
foreach ($p in $patterns) { $topics[$p.k] = 0 }
foreach ($v in $data) {
    $tl = $v.title.ToLower()
    foreach ($p in $patterns) { if ($tl -match $p.p) { $topics[$p.k]++ } }
}
$topics.GetEnumerator() | Sort-Object Value -Descending | ForEach-Object { Write-Host "  $($_.Key): $($_.Value)" }

Write-Host "`n=== TOP CREATORS (5+ videos) ===" -ForegroundColor Cyan
$ch = $data | Where-Object { $_.channel } | Group-Object { $_.channel }
$ch | Where-Object { $_.Count -ge 5 } | Sort-Object Count -Descending | ForEach-Object {
    $top = ($_.Group | Sort-Object { $v = $_.views -replace '[^0-9]',''; if ($v) {[int]$v} else {0} } -Descending | Select-Object -First 1)
    Write-Host "  $($_.Name): $($_.Count) videos"
    Write-Host "    Top: $($top.title) ($($top.views))" -ForegroundColor DarkGray
}

Write-Host "`n=== WEEKS ===" -ForegroundColor Cyan
$wks = @{}
foreach ($v in $data) {
    $pd = $v.published
    if (-not $pd) { continue }
    if ($pd -match '(\d+)\s*(day|week|month|year)') {
        $n = [int]$matches[1]
        $u = $matches[2]
        $mult = @{day=1;week=7;month=30;year=365}
        $dt = (Get-Date).AddDays(-($n * $mult[$u]))
        $wk = Get-Date $dt -Format "yyyy-MM-dd"
        if (-not $wks.ContainsKey($wk)) { $wks[$wk] = 0 }
        $wks[$wk]++
    } elseif ($pd -match 'hour') {
        $wk = Get-Date -Format "yyyy-MM-dd"
        if (-not $wks.ContainsKey($wk)) { $wks[$wk] = 0 }
        $wks[$wk]++
    }
}
$wks.GetEnumerator() | Sort-Object Name | Select-Object -Last 12 | ForEach-Object {
    $bar = "#" * [Math]::Min(50, $_.Value)
    Write-Host "  $($_.Name): $bar ($($_.Value))"
}

Write-Host "`n=== DATA SUMMARY ===" -ForegroundColor Green
Write-Host "Total videos: $total"
Write-Host "Unique queries: $(($data | Group-Object { $_.query }).Count)"
Write-Host "Unique channels: $($ch.Count)"
$wv = ($data | Where-Object { $_.views -and $_.views -ne '' }).Count
Write-Host "With views: $wv"
$wd = ($data | Where-Object { $_.published -and $_.published -ne '' }).Count
Write-Host "With dates: $wd"
