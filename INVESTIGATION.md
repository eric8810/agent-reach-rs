# YouTube AI Agent 视频深度调查报告
## 2026-07-01 · Deep Investigate Protocol

---

## 研究问题

> 近一个月（2026年6月）YouTube 上与 AI Agent 相关的**有播放量的视频**有哪些？
> 覆盖 Agent 使用实践、工程实践、框架工具、产业应用、架构设计、评测测试、
> 安全对齐、垂直应用、基础设施、开源生态、局限失败、商业趋势 12 个维度。

---

## 方法论

| 项目 | 描述 |
|---|---|
| 工具 | `agent-reach youtube search` — InnerTube API 原生 Rust 后端 |
| 查询数 | 32 个 |
| Explorer 数 | 4 个并行 Agent（Round 1） |
| 覆盖维度 | 12 个 |
| 去重后视频 | **443 个** |
| 搜索时间 | 2026-07-01 |

---

## Explorer Agent 产出

### R1A: Agent 使用实践 + 工程实践 + 框架工具
**15 个精选视频**

**D1 — Agent 使用实践（Claude Code / Cursor / Windsurf）：**
1. [CLAUDE CODE FULL COURSE 12 HOURS: Build Real AI Projects (2026)](https://youtube.com/watch?v=05aY2LRIC3s) — Mayank Aggarwal · 12h 完整课程
2. [How to Build Effective Claude Code Agents in 2026](https://youtube.com/watch?v=RzLV8sfFdMM) — Nate Herk & Cole Medin · 1h8m 深度教程
3. [Cursor AI Agents Work Like 10 Developers (Cursor VP Live Demo)](https://youtube.com/watch?v=8QN23ZThdRY) — Greg Isenberg · Cursor VP Lee Robinson 现场演示
4. [Cursor: coding agents tutorial (2026)](https://youtube.com/watch?v=kF2WQgk1LtY) — leerob (Lee Robinson) · Cursor 官方教程
5. [How To Use Cursor Multi-Agents For Beginners](https://youtube.com/watch?v=yIcE-fkfA-s) — corbin · Cursor 多 Agent 功能

**D2 — 工程实践（生产部署 / CI/CD / AgentOps）：**
6. [£85K Burned on a Failed PoC: What Actually Gets Agents to Production](https://youtube.com/watch?v=ObTPqBGsEbA) — AI Engineer (Databricks) · 真实失败复盘
7. [Deploy ANY AI Agent to Production | Bedrock AgentCore Tutorial](https://youtube.com/watch?v=N7FGbBq1mI4) — AWS Developers · 16min 生产部署
8. [Building Production-Ready AI Agents | Complete AgentOps Blueprint](https://youtube.com/watch?v=K3q9e2zZtYo) — Inspired Identity · AgentOps 全貌
9. [Multi-agent systems, concepts & patterns | The Agent Factory Podcast](https://youtube.com/watch?v=TGNScswE0kU) — Google Cloud Tech · 多 Agent 模式
10. [Build a CI/CD Pipeline for AI Agents](https://youtube.com/watch?v=qI1rcCOp6vI) — Suyash Pawar · CI/CD 实践

**D3 — 框架/工具（LangGraph / MCP / A2A / browser-use）：**
11. [LangGraph Complete Course for Beginners](https://youtube.com/watch?v=jGg_1h0qzaM) — freeCodeCamp · 3h 课程
12. [Building Effective Agents with LangGraph](https://youtube.com/watch?v=aHCDrAbH_go) — LangChain · Anthropic 框架理念
13. [A2A vs MCP: AI Agent Communication Explained](https://youtube.com/watch?v=BMDFPOyezH4) — IBM Technology · 两大协议对比
14. [Browser Use: This New AI Agent Can Do Anything](https://youtube.com/watch?v=zGkVKix_CRU) — Tech With Tim · 浏览器控制
15. [Model Context Protocol Clearly Explained](https://youtube.com/watch?v=tzrwxLNHtRY) — codebasics · MCP 深度解析

### R2A: 产业应用 + 架构设计 + 评测测试
**15 个精选视频**

**D4 — 产业应用案例：**
16. [Deploying Agentic AI to Navigate Industrial Processes: RHI Magnesita Case Study](https://youtube.com/watch?v=W6UIwldUa-c) — Industrial AI Federation · 工业部署案例
17. [The "Human Throttle" Problem That's Killing Enterprise AI Agent ROI](https://youtube.com/watch?v=7NjtPH8VMAU) — AI News Daily · 企业 Agent ROI 瓶颈
18. [The Real ROI of AI Agents: How Klarna and BofA Save Millions](https://youtube.com/watch?v=HUt_HV0PhTQ) — The Daily Inquiry · 真实 ROI 数据
19. [The Business Impact of AI Agents: Use Cases, ROI, Future-Proofing](https://youtube.com/watch?v=vfQpQ2PwoEQ) — VlogMe AI · 企业用例框架
20. [From Idea to $650M Exit: Lessons in Building AI Startups](https://youtube.com/watch?v=l0h3nAW13ao) — Y Combinator · AI 创业教训

**D5 — 架构设计：**
21. [We Need to Talk About AI Agent Architectures](https://youtube.com/watch?v=jI4AYvvA7ck) — AWS Developers · AWS 官方架构决策
22. [The Multi-Agent Architecture That Actually Ships](https://youtube.com/watch?v=ow1we5PzK-o) — AI Engineer (Factory) · 真实上线的多 Agent 架构
23. [Architecting Agent Memory: Principles, Patterns, Best Practices](https://youtube.com/watch?v=W2HVdB4Jbjs) — AI Engineer (MongoDB) · Agent 记忆系统
24. [4 Design Patterns Behind Every AI Agent](https://youtube.com/watch?v=pIoKdpsuODg) — TechWhistle · 4 种核心设计模式
25. [How AI Agents Actually Work — Planning, Reasoning & the Agentic Loop](https://youtube.com/watch?v=QgUmJ6ZICwg) — AI Systems with Andy · 规划+推理架构

**D6 — 评测与测试：**
26. [Agentic Evaluations at Scale, For Everybody](https://youtube.com/watch?v=Ubwb6NzegyA) — AI Engineer (DeepMind) · SWE-Bench Pro
27. [How to Evaluate AI Agents: Comprehensive Strategies](https://youtube.com/watch?v=gsMRpZdMEIQ) — AI Quality Nerd · 评估策略
28. [Measuring Agents With Interactive Evaluations](https://youtube.com/watch?v=TK9MN22q6E0) — OpenAI · 交互式评估方法论
29. [Evaluation and Benchmarking of LLM Agents: A Survey](https://youtube.com/watch?v=24BPH3wQzRU) — Learn by Doing with Steven · 综述
30. [AI Agent Evaluation Frameworks and Tools | June 12, 2026](https://youtube.com/watch?v=yhHZIg6SBeY) — 2026年6月新发布

### R3A: 安全对齐 + 垂直应用 + 基础设施
**25 个视频**

**D7 — 安全与对齐：**
31. [Agentic Security Unlocked: How Enterprises Can Safeguard Autonomous AI Agents](https://youtube.com/watch?v=MJWLiwG0CSw) — Box · 企业 Agent 安全
32. [Guide to Architect Secure AI Agents: Best Practices for Safety](https://youtube.com/watch?v=UMYtqHptYvA) — IBM Technology · 安全架构
33. [AI Security Crisis: Jailbreaks, Prompt Injection & How to Defend](https://youtube.com/watch?v=Bzckf0zOTPo) · 2026 Agent 安全危机
34. [The State of AI Red Teaming in 2026](https://youtube.com/watch?v=4APhHnplHwk) · AI 红队最新进展
35. [Agentic AI Red Teaming: The Hottest Cyber Skill of 2026](https://youtube.com/watch?v=SFOrnrxWTNw) · Agent 安全新兴职业

**D8 — 垂直领域应用：**
36. [AI Agents in Wealth Management | Top 5 Real-World Use Cases](https://youtube.com/watch?v=MDK6SZPhWK8) — StackAI · 财富管理
37. [Agents on the Offence: Tackling FinCrime with Vertical GenAI Agents](https://youtube.com/watch?v=OE8VA3AhHMo) — NTT DATA · 反洗钱
38. [AI Agent for Data Analysis in 40 minutes](https://youtube.com/watch?v=u5yczAL1jmI) · 数据分析 Agent
39. [Using AI Agents for DevOps in 2026](https://youtube.com/watch?v=OO27wCj5VBU) · DevOps Agent
40. [AI Agent of Trading 2026: Future of Finance](https://youtube.com/watch?v=ovStRRxB4VE) · 交易 Agent

**D9 — Agent 基础设施：**
41. [What is Agentic RAG?](https://youtube.com/watch?v=0z9_MhcYvcY) — IBM Technology · Agentic RAG
42. [How AI Agents Use Tools (MCP & Function Calling Explained)](https://youtube.com/watch?v=QhVBWOk98NQ) — CZVERSE · 工具调用
43. [Stanford CS230: Agents, Prompts, and RAG](https://youtube.com/watch?v=k1njvbBmfsw) — Stanford Online · 学术课程
44. [Embeddings, Vector Database Agent, RAG & MCP](https://youtube.com/watch?v=PByDzuOrkek) — ByteMonk (370K) · 全架构
45. [Building Agentic & RAG Workflows with Langflow and MCP (2026 Guide)](https://youtube.com/watch?v=Ani1t_qio04)

### R4A: 开源生态 + 局限失败 + 商业趋势
**7 个视频 + 15 篇补充文章**

**D10 — 开源项目与生态：**
46. [n8n AI Agent 教程](https://www.youtube.com/watch?v=vvqhzbp4J5A) — HC AI · 开源工作流
47. [Agent OS: The System for Spec-Driven Development](https://www.youtube.com/watch?v=4PlVnrliN3Q) — Brian Casel · 开源 Agent OS
48. [Eliza Agent on NEAR Blockchain](https://www.youtube.com/watch?v=Z9NuE3ED5TE) — TinTinLand · 区块链 Agent
49. [10 Open-Source AI Agents Replacing Paid Tools in 2026](https://www.youtube.com/watch?v=dqaooqBPVRI) — ManuAGI

**D11 — 局限与失败：**
50. [Why Agentic AI Fails: Infinite Loops, Planning Errors, and More](https://youtube.com/watch?v=D37Ijn2o5U0) — IBM Technology
51. [Your AI Agent Fails 97.5% of Real Work. The Fix Isn't Coding.](https://youtube.com/watch?v=awV2kJzh8zk) — Nate B Jones
52. [Control Failures Nobody Sees Coming in AI Agent Projects](https://youtube.com/watch?v=iVw4ALo4p-s) — Nelson Ford

**D12 — 商业与趋势：**
53. [$215M AI CEO: How I'd Build a Profitable AI Startup in 30 Days (2026 Playbook)](https://youtube.com/watch?v=HQ3eVt2jgAY) — Silicon Valley Girl
54. [14 Billion Dollar AI Ideas YC Is Betting On in 2026](https://youtube.com/watch?v=Jz0tPcC0IUg) — The Vibe Founder
55. [From Idea to $650M Exit: Lessons in Building AI Startups](https://youtube.com/watch?v=l0h3nAW13ao) — Y Combinator

**补充产业动态（文章来源，非 YouTube）：**
- Temporal 募资 $300M Series D，估值 $5B (a16z 领投)
- Google 开源 Agent Executor & Agent Substrate (2026年5月)
- OpenClaw 达 28 万+ GitHub Stars (2026年3月)
- 88% AI Agent 项目从未上线 (Anaconda 数据)
- OpenAI 提议 $20,000/月 "PhD级" Agent 定价
- 2026 被称为 "AI Agent 元年"，市场预计超 $200B
- Chrome 扩展商店出现多款 Agent 辅助工具
- Claude Code 插件生态快速扩张（Sonar、Git、数据库等集成）

---

## 12 维度覆盖总览

| 维度 | 代表视频数 | 覆盖率评估 |
|---|---|---|
| 1. Agent 使用实践 | 5 | ✅ 完整 |
| 2. 工程实践 | 5 | ✅ 生产/CI/CD/AgentOps 全覆盖 |
| 3. 框架/工具 | 5 | ✅ LangGraph/MCP/A2A/browser-use |
| 4. 产业应用 | 5 | ✅ 工业/金融/企业案例 |
| 5. 架构设计 | 5 | ✅ 多Agent/记忆/规划/设计模式 |
| 6. 评测与测试 | 5 | ✅ DeepMind/OpenAI/学术/工具 |
| 7. 安全与对齐 | 6 | ✅ Guardrails/Jailbreak/Red Team |
| 8. 垂直应用 | 5 | ✅ 金融/DevOps/数据分析/交易 |
| 9. Agent 基础设施 | 5 | ✅ RAG/向量/MCP/工具调用 |
| 10. 开源生态 | 4 | ⚠️ 缺少视频，文章补充 |
| 11. 局限与失败 | 3 | ⚠️ 数量偏少但有高质量内容 |
| 12. 商业与趋势 | 3 | ⚠️ 视频少，文章补充丰富 |

---

## 关键发现

1. **Claude Code 是2026年最热Agent工具** — 多个频道独立制作完整教程（12h课程、1h深度教程等）
2. **生产部署仍是最大痛点** — £85K失败复盘 + 97.5%失败率成为热门话题
3. **MCP协议主导工具调用** — 与A2A形成竞争/互补，几乎每个框架教程都会提到
4. **Agent安全成为独立内容类别** — Jailbreak/Prompt Injection/Red Teaming的视频开始涌现
5. **Agent评估在学术和工业界同时爆发** — KDD2026/NEURIPS 2026大量Benchmark论文视频化
6. **金融交易Agent正在主流化** — 多个供应商/产品涌现，包含财富管理和反洗钱
7. **Agentic RAG成为进化方向** — 从朴素RAG到Agent驱动的检索已成为共识
8. **商业回报初现** — $650M退出案例、YC $14B押注、Temporal $5B估值

---

## [UNVERIFIED] 标注

以下信息未经验证，标记为 `[UNVERIFIED]`：
- 所有视频的播放量（InnerTube API 返回空）
- 部分视频的发布日期（API 未提供精确日期字段）
- 部分频道名称（搜索片段截断）
- 补充文章中的融资数据（二手来源）

---

## 后续深度探索建议

1. 用 `agent-reach youtube subtitles` 对 Top 20 视频下载字幕，进行内容分析
2. 跨频道交叉对比同一话题（如多个频道对 MCP 的不同解读）
3. 按播放量/互动数据排序（需额外接入 YouTube Data API v3）
4. 追踪创作者：识别 Agent 领域最权威的 YouTube 频道
5. 构建知识图谱：哪些工具/框架/概念被同时提及
