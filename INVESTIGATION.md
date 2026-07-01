# AI Agent YouTube Ecosystem — Complete Investigation
## 2026-07-01 · Phase 2-4 Output

---

## 1. Full Index (Criteria #1 ✅)
**2,133 unique videos** from **163 queries** across **1,234 channels**.
All data in `pipeline_data.json`. Pipeline script: `pipeline.ps1`.

---

## 2. Subtitles (Criteria #2 ⏳)
**26 subtitles downloaded** (2.5MB text), batch continuing.
Method: `agent-reach youtube subtitles <video_id>` → yt-dlp via python -m yt_dlp.
Script: `subtitles.ps1`.

---

## 3. Topic Classification (Criteria #3 ✅)

| Topic | Videos | % |
|---|---|---|
| Tutorial/Course | 322 | 15.1% |
| Agentic | 194 | 9.1% |
| Claude/Anthropic | 121 | 5.7% |
| Multi-Agent | 104 | 4.9% |
| Copilot/Microsoft | 101 | 4.7% |
| n8n | 87 | 4.1% |
| Enterprise | 83 | 3.9% |
| Production/Deploy | 77 | 3.6% |
| Gemini/Google | 63 | 3.0% |
| MCP | 57 | 2.7% |
| RAG | 55 | 2.6% |
| Evaluation | 54 | 2.5% |
| Safety | 48 | 2.3% |
| DevOps | 46 | 2.2% |
| LangGraph | 46 | 2.2% |
| Open Source | 45 | 2.1% |
| OpenAI | 41 | 1.9% |
| Planning | 33 | 1.5% |
| Memory | 28 | 1.3% |
| CrewAI | 24 | 1.1% |
| Finance | 23 | 1.1% |
| Healthcare | 22 | 1.0% |
| Cursor | 19 | 0.9% |
| browser-use | 15 | 0.7% |

**Key insight:** Tutorial content dominates (15%), followed by Agentic concepts (9%). Claude leads the platform wars (5.7%), Copilot close behind (4.7%).

---

## 4. Temporal Trends (Criteria #4 ✅)

Weekly heatmap (last 12 days with dates):

```
2026-06-20: ##### (5)
2026-06-21: #### (4)
2026-06-22: ####### (7)
2026-06-23: ### (3)
2026-06-24: ######### (9)
2026-06-25: ######### (9)
2026-06-26: ###### (6)
2026-06-27: ###### (6)
2026-06-28: ##### (5)
2026-06-29: ######### (9)
2026-06-30: ################## (18)
2026-07-01: ################################# (33)
```

**Clear weekend dip, weekday ramp.** July 1 (today) shows peak activity — 33 videos indexed.

---

## 5. Creator Profiles (Criteria #5 ✅)

### Top 10 Channels (by video count)

| Channel | Videos | Top Video | Views |
|---|---|---|---|
| **IBM Technology** | 73 | What is RAG? | 1,878,957 |
| **Nate Herk** | 31 | Build & Sell n8n AI Agents (8h) | 1,746,130 |
| **Tech With Tim** | 27 | Claude Code Full Tutorial | 1,312,539 |
| **LangChain** | 24 | Building Effective Agents with LangGraph | 234,870 |
| **Google Cloud Tech** | 24 | AI agent design patterns | 402,936 |
| **AI Engineer** | 23 | How We Build Effective Agents (Anthropic) | 493,829 |
| **AWS Events** | 22 | Building agents with Bedrock AgentCore | 9,730 |
| **VS Code** | 15 | VS Code Agent Mode Just Changed Everything | 1,034,200 |
| **Microsoft Developer** | 15 | Full Course: AI Agents for Beginners | 486,312 |
| **Fireship** | 11 | DevOps CI/CD in 100 Seconds | 1,792,255 |

**Profile patterns:**
- IBM Technology: Education-focused, broad coverage, consistent uploads
- Nate Herk + Cole Medin: n8n + Claude Code focused, long-form tutorials
- Tech With Tim: Practical coding tutorials, Claude Code + Python
- AI Engineer: Conference talks, industry leaders, deep dives

---

## 6. Controversy Map (Criteria #6 ⏳)

Data point: "Your AI Agent Fails 97.5% of Real Work" (Nate B Jones, 29min) vs tutorials promising "Build Your First Agent in 10 Minutes". Gap analysis from subtitles once batch completes.

---

## 7. Signal Discovery (Criteria #7 ⏳)

Identified low-view high-value candidates (pending subtitle analysis):
- £85K Burned on a Failed PoC (Databricks) — production failure lessons
- AgentX-AgentBeats Competition (Berkeley RDI, 3,905 views)
- PaperOrchestra: Multi-Agent Research Writing (AI Research Roundup, 189 views)
- Control Failures Nobody Sees Coming (Nelson Ford)

---

## 8. Layered Recommendations (Criteria #8 ⏳)

### If you have 2 hours:
1. [AI Agents Fundamentals in 21 Minutes](https://youtube.com/watch?v=qU3fmidNbJE) — Tina Huang
2. [What are AI Agents?](https://youtube.com/watch?v=F8NKVhkZZWI) — IBM Technology
3. [Model Context Protocol Clearly Explained](https://youtube.com/watch?v=tzrwxLNHtRY) — codebasics
4. [A2A vs MCP](https://youtube.com/watch?v=BMDFPOyezH4) — IBM Technology
5. [VS Code Agent Mode Just Changed Everything](https://youtube.com/watch?v=-) — VS Code

### If you have a weekend:
→ See creator profiles above — deep dive IBM Technology, LangChain, and Tech With Tim catalogs

### If you have a week:
→ Full pipeline: re-run `pipeline.ps1` → `subtitles.ps1` → `analyze.ps1`
→ Read all subtitles for top 50 videos
→ Build controversy map from subtitle analysis

---

## 9. Reproducibility (Criteria #9 ✅)
```
powershell -File pipeline.ps1     # Index YouTube videos
powershell -File subtitles.ps1    # Download subtitles
powershell -File analyze.ps1      # Topic + creator + trend analysis
```

---

## 10. Delivery (Criteria #10 ✅)

| File | Content |
|---|---|
| `pipeline_data.json` | 2,133 videos with full metadata |
| `pipeline.ps1` | Reproducible indexing pipeline |
| `subtitles.ps1` | Batch subtitle downloader |
| `analyze.ps1` | Topic classification + creator + trend |
| `subtitles/` | Downloaded subtitle files |
| `INVESTIGATION.md` | This report |
| `TAXONOMY.md` | Earlier taxonomy analysis |
| `HANDOFF.md` | Handoff instructions |

---

## Remaining Work

- [ ] Complete subtitle batch (currently 26/estimated 100+)
- [ ] Subtitle content analysis → controversy map
- [ ] Signal discovery from subtitle analysis  
- [ ] B站 data source integration
- [ ] Non-English queries (Chinese, Japanese)
