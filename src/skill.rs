//! Agent skill installation/uninstallation.
//!
//! Installs/uninstalls the agent SKILL.md file to known agent skill directories
//! so that AI agents (OpenClaw, Claude Code, generic .agents) can discover and
//! use agent-reach capabilities.
//!
//! Ported from Python `_install_skill()` / `_uninstall_skill()` in
//! `agent_reach/cli.py`.

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

// ── embedded skill content ────────────────────────────────────────────────

/// Main SKILL.md content in Chinese (default).
const SKILL_CONTENT: &str = r#"---
name: agent-reach
description: >
  MUST USE when user wants to 调研/research/搜索/search/查/找/look up anything
  on the internet — e.g. 全网调研 X / 帮我调研一下 X / 查一下 X / 搜搜 X /
  看看大家怎么评价 X / X 上有什么讨论 / research this topic.

  Also MUST USE when user mentions any platform or shares any URL/链接:
  小红书/xiaohongshu/xhs, Twitter/推特/X, B站/bilibili, Reddit, V2EX,
  LinkedIn/领英/招聘/求职/jobs, YouTube, GitHub code search, 小宇宙播客,
  雪球/股票行情, RSS feeds, or any web URL.

  13 platforms, multi-backend routing (OpenCLI / per-platform CLIs / APIs).
  Zero config for 6 channels. Run `agent-reach doctor --json` to see which
  backend serves each platform right now.

  NOT for: 写报告/数据分析/翻译等内容加工（本 skill 只负责从互联网获取内容）；
  发帖/评论/点赞等写操作；已有专门 skill 的平台（先用专门 skill）。

  【路由方式】SKILL.md 包含路由表和常用命令，复杂场景需按需阅读对应分类的 references/*.md。
  分类：search / social (小红书/推特/B站/V2EX/Reddit) / career(LinkedIn) / dev(github) / web(网页/文章/RSS) / video(YouTube/B站/播客)。
triggers:
  - research: 调研/全网调研/帮我调研/研究一下/research/深入了解
  - search: 搜/查/找/search/搜索/查一下/帮我搜/看看大家怎么说
  - social:
    - 小红书: xiaohongshu/xhs/小红书/红书
    - Twitter: twitter/推特/x.com/推文
    - B站: bilibili/b站/哔哩哔哩
    - V2EX: v2ex
    - Reddit: reddit
  - career: 招聘/职位/求职/linkedin/领英/找工作
  - dev: github/代码/仓库/gh/issue/pr/分支/commit
  - web: 网页/链接/文章/rss/读一下/打开这个
  - video: youtube/视频/播客/字幕/小宇宙/转录/yt
  - finance: 雪球/股票/stock/xueqiu/行情/基金
metadata:
  openclaw:
    homepage: https://github.com/Panniantong/Agent-Reach
---

# Agent Reach — 互联网能力路由器

13 平台、多后端。**本 skill 存在时必须用它访问这些平台，不要自己发明方案。**

## 常驻规则（全程适用）

1. **动手前先体检**：多后端平台（小红书/Reddit/B站/Twitter）先跑
   `agent-reach doctor --json`，按各平台 `active_backend` 字段选命令组。
2. **声明你在用什么**：开始干活前说一句「使用 agent-reach 的 X 平台 / Y 后端」。
3. **失败按 references 里的重试链处理**，不要瞎猜命令。
4. **全网调研类任务**：组合多平台（Exa 搜索 + Twitter/Reddit 看讨论 + 小红书/B站看中文场景），并行收集再汇总。
5. **替用户盯版本**：完成一次较大的调研/多平台任务后，顺手跑
   `agent-reach check-update`（很快，一个 API 调用）。有新版就在收尾汇报里附一句：
   「Agent Reach 有新版 vX.Y.Z，复制这句话给我即可更新：帮我更新 Agent Reach：
   https://raw.githubusercontent.com/Panniantong/agent-reach/main/docs/update.md」。
   不要中断当前任务去更新，也不要重复提醒同一个版本。

## 路由表

| 用户意图 | 分类 | 详细文档 |
|---------|------|---------|
| 网页搜索/代码搜索 | search | [references/search.md](references/search.md) |
| 小红书/推特/B站/V2EX/Reddit | social | [references/social.md](references/social.md) |
| 招聘/职位/LinkedIn | career | [references/career.md](references/career.md) |
| GitHub/代码 | dev | [references/dev.md](references/dev.md) |
| 网页/文章/RSS | web | [references/web.md](references/web.md) |
| YouTube/B站/播客字幕 | video | [references/video.md](references/video.md) |

## 零配置快速命令

```bash
# Exa 网页搜索
mcporter call 'exa.web_search_exa(query: "query", numResults: 5)'

# 通用网页阅读
curl -s "https://r.jina.ai/URL"

# GitHub 搜索
gh search repos "query" --sort stars --limit 10

# YouTube 字幕（注意：B站不要用 yt-dlp，见 video.md）
yt-dlp --write-sub --skip-download -o "/tmp/%(id)s" "URL"

# V2EX 热门
curl -s "https://www.v2ex.com/api/topics/hot.json" -H "User-Agent: agent-reach/1.0"

# B站搜索（bili-cli，无需登录）
bili search "query" --type video -n 5
```

## 需登录态的平台（按 doctor 的 active_backend 选命令）

```bash
# Twitter 搜索（twitter-cli 首选；失败重试链见 social.md）
twitter search "query" -n 10

# Reddit（无零配置路径：OpenCLI 或 rdt-cli，必须登录态）
opencli reddit search "query" -f yaml   # 桌面
rdt search "query" --limit 10            # 存量/服务器

# 小红书（桌面首选 OpenCLI）
opencli xiaohongshu search "query" -f yaml
```

## 环境检查

```bash
# 检查可用 channel 与每个平台当前激活的后端
agent-reach doctor --json
```

## 工作区规则

**不要在 agent workspace 创建文件。** 使用 `/tmp/` 存放临时输出，`~/.agent-reach/` 存放持久数据。

## 详细文档

根据用户需求，阅读对应的详细文档：

- [搜索工具](references/search.md) — Exa AI 搜索
- [社交媒体](references/social.md) — 小红书, Twitter, B站, V2EX, Reddit（多后端命令组）
- [职场招聘](references/career.md) — LinkedIn
- [开发工具](references/dev.md) — GitHub CLI
- [网页阅读](references/web.md) — Jina Reader, RSS
- [视频播客](references/video.md) — YouTube, B站, 小宇宙

## 配置渠道

如果某个 channel 需要配置，获取安装指南：
https://raw.githubusercontent.com/Panniantong/agent-reach/main/docs/install.md

用户只需提供 cookies，其他配置由 agent 完成。
"#;

/// English SKILL.md content.
const SKILL_CONTENT_EN: &str = r#"---
name: agent-reach
description: >
  MUST USE when user wants to research/search/look up/find anything on the
  internet — e.g. "research this topic", "do a deep dive on X", "search the
  web for X", "see what people say about X", "look this up".

  Also MUST USE when user mentions any platform or shares any URL/link:
  Twitter/X, Reddit, YouTube, GitHub, Bilibili, XiaoHongShu,
  Xiaoyuzhou Podcast, LinkedIn/jobs/recruiting, V2EX, Xueqiu (stocks), RSS.

  13 platforms, multi-backend routing (OpenCLI / per-platform CLIs / APIs).
  Zero config for 6 channels. Run `agent-reach doctor --json` to see which
  backend serves each platform right now.

  NOT for: writing reports/analysis/translation (this skill only FETCHES
  internet content); posting/commenting/liking (write operations); platforms
  that already have a dedicated skill installed (prefer that skill).
metadata:
  openclaw:
    homepage: https://github.com/Panniantong/Agent-Reach
---

# Agent Reach — internet capability router

13 platforms, multiple backends each. **When this skill exists, use it for
these platforms — do not invent your own approach.**

## Standing rules (apply for the whole session)

1. **Health-check before acting**: for multi-backend platforms (XiaoHongShu /
   Reddit / Bilibili / Twitter), run `agent-reach doctor --json` first and
   pick the command group matching each platform's `active_backend`.
2. **Announce what you use**: say "using agent-reach, platform X via backend Y"
   before starting.
3. **On failure, follow the retry chains in references/** — never guess
   commands.
4. **For broad research tasks**: combine platforms (Exa for web search +
   Twitter/Reddit for discussions + XiaoHongShu/Bilibili for Chinese
   perspectives), collect in parallel, then synthesize.
5. **Watch versions for the user**: after finishing a substantial
   multi-platform task, run `agent-reach check-update` (fast, one API call).
   If a new version exists, append one line to your wrap-up: "Agent Reach
   vX.Y.Z is available — paste this to me to update: 帮我更新 Agent Reach：
   https://raw.githubusercontent.com/Panniantong/agent-reach/main/docs/update.md".
   Never interrupt the current task to update; never nag about the same version twice.

## Routing table

| User intent | Category | Details |
|---------|------|---------|
| Web / code search | search | [references/search.md](references/search.md) |
| XiaoHongShu / Twitter / Bilibili / V2EX / Reddit | social | [references/social.md](references/social.md) |
| Jobs / LinkedIn | career | [references/career.md](references/career.md) |
| GitHub / code | dev | [references/dev.md](references/dev.md) |
| Web pages / articles / RSS | web | [references/web.md](references/web.md) |
| YouTube / Bilibili / podcast transcripts | video | [references/video.md](references/video.md) |

## Zero-config quick commands

```bash
# Exa web search
mcporter call 'exa.web_search_exa(query: "query", numResults: 5)'

# Read any web page
curl -s "https://r.jina.ai/URL"

# GitHub search
gh search repos "query" --sort stars --limit 10

# YouTube subtitles (NOTE: never use yt-dlp for Bilibili — see video.md)
yt-dlp --write-sub --skip-download -o "/tmp/%(id)s" "URL"

# V2EX hot topics
curl -s "https://www.v2ex.com/api/topics/hot.json" -H "User-Agent: agent-reach/1.0"

# Bilibili search (bili-cli, no login needed)
bili search "query" --type video -n 5
```

## Login-backed platforms (pick by doctor's active_backend)

```bash
# Twitter search (twitter-cli preferred; retry chain in social.md)
twitter search "query" -n 10

# Reddit (NO zero-config path — OpenCLI or rdt-cli, login required)
opencli reddit search "query" -f yaml   # desktop
rdt search "query" --limit 10            # legacy/server

# XiaoHongShu (desktop prefers OpenCLI)
opencli xiaohongshu search "query" -f yaml
```

## Environment check

```bash
# Channel availability + which backend serves each platform
agent-reach doctor --json
```

## Workspace rules

**Never create files in the agent workspace.** Use `/tmp/` for temporary
output and `~/.agent-reach/` for persistent data.

## Detailed references

Read the matching file when you need specifics (commands above cover the
common cases; references hold per-backend command groups, caveats, retry
chains — note: reference docs are written in Chinese, commands are universal):

- [Search](references/search.md) — Exa AI search
- [Social](references/social.md) — XiaoHongShu, Twitter, Bilibili, V2EX, Reddit (multi-backend groups)
- [Career](references/career.md) — LinkedIn
- [Dev](references/dev.md) — GitHub CLI
- [Web](references/web.md) — Jina Reader, RSS
- [Video](references/video.md) — YouTube, Bilibili, Xiaoyuzhou

## Configure a channel

If a channel needs setup, fetch the install guide:
https://raw.githubusercontent.com/Panniantong/agent-reach/main/docs/install.md

The user only provides cookies / one extension click; the agent does the rest.
"#;

/// Embedded reference files: (filename, content).
const REF_SEARCH: &str = r#"# 搜索工具

Exa AI 搜索引擎。

## Exa AI 搜索

高质量 AI 搜索引擎，擅长技术和代码搜索。

```bash
mcporter call 'exa.web_search_exa(query: "query", numResults: 5)'
mcporter call 'exa.get_code_context_exa(query: "code question", tokensNum: 3000)'
```

### 使用场景

| 场景 | 参数 |
|-----|------|
| 网页搜索 | `web_search_exa(query: "...", numResults: 5)` |
| 代码搜索 | `get_code_context_exa(query: "...", tokensNum: 3000)` |

### 特点

- 擅长英文内容和技术文档
- 支持代码上下文搜索
- 结果质量高

## 与其他搜索工具对比

| 工具 | 来源 | 适用场景 |
|-----|------|---------|
| Exa | agent-reach | 英文/技术/代码搜索 |
| 智谱搜索 | my-mcp-tools | 中文搜索 |
| GitHub 搜索 | agent-reach (dev.md) | 仓库/代码搜索 |
"#;

const REF_SOCIAL: &str = r#"# 社交媒体 & 社区

小红书、Twitter/X、B站、V2EX、Reddit。

## 小红书 / XiaoHongShu（多后端）

小红书有三个后端，**先跑 `agent-reach doctor --json` 看 xiaohongshu 的 `active_backend` 是哪个**，再用对应命令组。

### 后端 A：OpenCLI（桌面首选，复用浏览器登录态）

```bash
# 搜索笔记
opencli xiaohongshu search "query" -f yaml

# 读笔记正文+互动数据（用搜索结果里的完整 URL，含 xsec_token）
opencli xiaohongshu note "NOTE_URL" -f yaml

# 评论（支持楼中楼）
opencli xiaohongshu comments NOTE_ID -f yaml

# 首页推荐 feed
opencli xiaohongshu feed -f yaml

# 用户主页公开笔记
opencli xiaohongshu user USER_ID -f yaml
```

> 要求 Chrome 打开且装了 OpenCLI 扩展。报 AUTH_REQUIRED 说明浏览器里没登录小红书，让用户在 Chrome 里登录一次即可。

### 后端 B：xiaohongshu-mcp（服务器场景）

```bash
# 未登录时：先查状态，再取二维码给用户扫
mcporter call 'xiaohongshu.check_login_status()' --timeout 120000
mcporter call 'xiaohongshu.get_login_qrcode()' --timeout 120000

# 搜索
mcporter call 'xiaohongshu.search_feeds(keyword: "query")' --timeout 120000

# 笔记详情+评论（feed_id 和 xsec_token 从搜索结果取）
mcporter call 'xiaohongshu.get_feed_detail(feed_id: "...", xsec_token: "...")' --timeout 120000
```

> 首次调用会自动下载约 150MB 无头浏览器，务必带 `--timeout 120000`。未登录时 search 会挂死，先 check_login_status。

### 后端 C：xhs-cli（存量备选，上游 2026-03 起停更）

```bash
xhs search "query"          # 搜索
xhs read NOTE_ID_OR_URL     # 读笔记（必须用搜索结果中的 URL/ID，不能裸 note_id）
xhs comments NOTE_ID_OR_URL # 评论
xhs hot                     # 热门
xhs feed                    # 推荐
```

> 已知不稳定：`xhs user` / `xhs user-posts` / `xhs favorites` 可能返回 API error（上游停更无人修）。新装用户建议直接走后端 A/B。

### 通用注意事项

> **xsec_token 限制**: 小红书强制 xsec_token 机制，**不能直接用裸 note_id 去读**。正确流程：先搜索/feed 拿结果，再用结果中的完整 URL/ID 去读。三个后端都一样。
>
> **频率控制**: 高频请求（批量搜索、深翻评论）会触发验证码，平台限制无法绕过。每次操作间隔 2-3 秒。
>
> **写操作（发帖/评论/点赞）**: 建议只读。xhs-cli v0.6.x 写操作可能因签名问题返回 406。

## Twitter/X (twitter-cli)

### 稳定命令

```bash
# 首页时间线（最稳定）
twitter feed -n 20

# 读取单条推文（含回复）
twitter tweet URL_OR_ID

# 读取长文 / X Article
twitter article URL_OR_ID

# 用户时间线
twitter user-posts @username -n 20

# 用户资料
twitter user @username
```

### 可能不稳定的命令

```bash
# 搜索推文（Twitter 频繁改 GraphQL 端点，可能 404）
twitter search "query" -n 10

# likes（2024 年后只能看自己的，平台限制）
twitter likes
```

### search 失败时的重试链（按序执行，成功即停）

1. 直接重试一次（偶发失败常见）：`twitter search "query" -n 10`
2. 升级后再试：`pipx upgrade twitter-cli && twitter search "query" -n 10`
3. 换 OpenCLI 备选（桌面，复用浏览器登录态）：`opencli twitter search "query" -f yaml`
4. 都不行就改用 `twitter feed` / `twitter user-posts @somebody` 等稳定命令绕路

### 重要注意事项

> **安装**: `pipx install twitter-cli`（确保 v0.8.5+）
>
> **认证**: 推荐用 Cookie-Editor 导出后设置环境变量 `TWITTER_AUTH_TOKEN` + `TWITTER_CT0`。自动提取在 SSH/Docker/无头环境不可用。
>
> **IP 风控**: 不要在 VPS/数据中心 IP 上频繁调用，尤其是 followers/following，有封号风险。使用住宅代理或本地环境。
>
> **OpenCLI 备选**: 桌面装了 OpenCLI 的话，`opencli twitter search/article/user-posts -f yaml` 全套可用（浏览器登录态，无需 cookie 环境变量）。
>
> **输出格式**: 建议用 `--yaml` 或 `--json` 获得结构化输出，对 AI agent 更友好。

## B站 / Bilibili

> ⚠️ **不要用 yt-dlp 读 B站**（风控已全面 412 拦截，实测无解）。用 bili-cli / OpenCLI。

```bash
# 搜索 / 热门 / 视频详情（bili-cli，只读无需登录）
bili search "query" --type video -n 5
bili hot -n 10
bili video BVxxx

# 字幕（OpenCLI，需桌面 Chrome）
opencli bilibili subtitle BVxxx
```

> 详细命令（音频转写、API 直连兜底）见 [references/video.md](video.md)。

## V2EX (公开 API)

无需认证，直接调用公开 API。

### 热门主题

```bash
curl -s "https://www.v2ex.com/api/topics/hot.json" -H "User-Agent: agent-reach/1.0"
```

### 节点主题

```bash
# node_name 如: python, tech, jobs, qna, programmers
curl -s "https://www.v2ex.com/api/topics/show.json?node_name=python&page=1" -H "User-Agent: agent-reach/1.0"
```

### 主题详情

```bash
# topic_id 从 URL 获取，如 https://www.v2ex.com/t/1234567
curl -s "https://www.v2ex.com/api/topics/show.json?id=TOPIC_ID" -H "User-Agent: agent-reach/1.0"
```

### 主题回复

```bash
curl -s "https://www.v2ex.com/api/replies/show.json?topic_id=TOPIC_ID&page=1" -H "User-Agent: agent-reach/1.0"
```

### 用户信息

```bash
curl -s "https://www.v2ex.com/api/members/show.json?username=USERNAME" -H "User-Agent: agent-reach/1.0"
```

### Python 调用示例

```python
from agent_reach.channels.v2ex import V2EXChannel

ch = V2EXChannel()

# 获取热门帖子
topics = ch.get_hot_topics(limit=10)
for t in topics:
    print(f"[{t['node_title']}] {t['title']} ({t['replies']} 回复)")

# 获取节点帖子
node_topics = ch.get_node_topics("python", limit=5)

# 获取帖子详情 + 回复
topic = ch.get_topic(1234567)
print(topic["title"], "—", topic["author"])

# 获取用户信息
user = ch.get_user("Livid")
```

> **节点列表**: https://www.v2ex.com/planes

## Reddit（多后端，必须登录态）

**Reddit 没有零配置路径**：匿名 `.json` 端点已被封（403），官方 API 自 2025-11 起人工审批基本不批。两个后端都靠登录态，先跑 `agent-reach doctor --json` 看 reddit 的 `active_backend`。中国大陆访问需代理。

### 后端 A：OpenCLI（桌面首选，复用浏览器登录态）

```bash
# 搜索帖子
opencli reddit search "query" -f yaml

# 读帖子全文 + 评论
opencli reddit read POST_ID -f yaml

# 浏览 subreddit / 热门 / Popular
opencli reddit subreddit LocalLLaMA -f yaml
opencli reddit hot -f yaml
opencli reddit popular -f yaml

# subreddit 元信息（订阅数、简介）
opencli reddit subreddit-info LocalLLaMA -f yaml
```

> 要求 Chrome 打开且浏览器里登录过 reddit.com。

### 后端 B：rdt-cli（存量/服务器备选，上游 2026-03 起停更）

```bash
rdt search "query" --limit 10   # 搜索帖子
rdt read POST_ID                # 读帖子全文 + 评论
rdt sub python --limit 20       # 浏览 subreddit
rdt popular --limit 10          # 浏览热门
rdt all --limit 10              # 浏览 /r/all
```

> **安装**: `pipx install 'git+https://github.com/public-clis/rdt-cli.git'`（PyPI 版本落后，需从 GitHub 装 v0.4.2+）。先 `rdt login` 才能搜索和阅读（服务器无浏览器时手动写 Cookie，见 doctor 提示）。
> 建议使用 `--yaml` 输出，对 AI agent 更友好。

### 高级选项：官方 API + PRAW（仅限已有凭证的用户）

2025-11 前注册过 Reddit script app（持有 client_id/client_secret）的用户可以用 PRAW 走官方 API（100 QPM 免费）。新申请需人工审批且个人项目基本不批，**不要推荐新用户走这条路**。
"#;

const REF_CAREER: &str = r#"# 职场招聘

LinkedIn。

## LinkedIn

```bash
# 获取个人资料
mcporter call 'linkedin-scraper.get_person_profile(linkedin_url: "https://linkedin.com/in/username")'

# 搜索人才
mcporter call 'linkedin-scraper.search_people(keyword: "AI engineer", limit: 10)'

# 获取公司资料
mcporter call 'linkedin-scraper.get_company_profile(linkedin_url: "https://linkedin.com/company/xxx")'

# 搜索职位
mcporter call 'linkedin-scraper.search_jobs(keyword: "software engineer", limit: 10)'
```

> **需要登录**: LinkedIn scraper 需要有效的登录态。

### Fallback 方案

如果 MCP 不可用，可以用 Jina Reader：

```bash
curl -s "https://r.jina.ai/https://linkedin.com/in/username"
```
"#;

const REF_DEV: &str = r#"# 开发工具

GitHub CLI 

## GitHub (gh CLI)

GitHub 官方命令行工具，用于仓库、Issue、PR、Actions、Release 以及 API 访问。

```bash
# 认证
gh auth login
gh auth status

# 搜索
gh search repos "query" --sort stars --limit 10
gh search code "query" --language python

# 仓库
gh repo view owner/repo
gh repo clone owner/repo
gh repo create my-repo --private
gh repo fork owner/repo
gh repo fork owner/repo --clone
gh repo sync owner/repo

# Issues
gh issue list -R owner/repo --state open
gh issue view 123 -R owner/repo
gh issue create -R owner/repo --title "Title" --body "Body"

# Pull Requests
gh pr list -R owner/repo --state open
gh pr view 123 -R owner/repo
gh pr create -R owner/repo --title "Title" --body "Body"
gh pr checks 123 --repo owner/repo

# Actions / CI
gh run list --repo owner/repo --limit 10
gh run view <run-id> --repo owner/repo
gh run view <run-id> --repo owner/repo --log-failed
gh workflow list --repo owner/repo

# Releases
gh release list -R owner/repo
gh release create v1.0.0

# API
gh api /user
gh api repos/owner/repo

# JSON 输出
gh issue list --repo owner/repo --json number,title --jq '.[] | "\(.number): \(.title)"'
```


## 选择指南

| 工具 | 来源 | 用途 |
|-----|------|------|
| gh CLI | agent-reach | Git 操作 |
| zread | my-mcp-tools | 读仓库内容 |
| context7 | my-mcp-tools | 查技术文档 |
"#;

const REF_WEB: &str = r#"# 网页阅读

通用网页、RSS。

## 通用网页 (Jina Reader)

```bash
# 读取任意网页内容
curl -s "https://r.jina.ai/URL"

# 示例
curl -s "https://r.jina.ai/https://example.com/article"
```

**适用场景**: 大多数网页可以直接用 Jina Reader 读取。

## Web Reader (MCP)

```bash
# 读取网页内容 (Markdown 格式)
mcporter call 'web-reader.webReader(url: "https://example.com")'

# 保留图片
mcporter call 'web-reader.webReader(url: "https://example.com", retain_images: true)'

# 纯文本格式
mcporter call 'web-reader.webReader(url: "https://example.com", return_format: "text")'
```

**适用场景**: 需要更精确控制输出格式时使用。

## RSS (feedparser)

```python
python3 -c "
import feedparser
for e in feedparser.parse('FEED_URL').entries[:5]:
    print(f'{e.title} — {e.link}')
"
```

**适用场景**: 订阅博客、新闻源、播客等 RSS feed。

## 选择指南

| 场景 | 推荐工具 |
|-----|---------|
| 通用网页 | Jina Reader (`curl r.jina.ai`) |
| 需要图片/格式控制 | web-reader MCP |
| RSS 订阅 | feedparser |
"#;

const REF_VIDEO: &str = r##"# 视频/播客

YouTube、B站、小宇宙播客的字幕和转录。

## YouTube (yt-dlp)

### 获取视频元数据

```bash
yt-dlp --dump-json "URL"
```

### 下载字幕

```bash
# 下载字幕 (不下载视频)
yt-dlp --write-sub --write-auto-sub --sub-lang "zh-Hans,zh,en" --skip-download -o "/tmp/%(id)s" "URL"

# 然后读取 .vtt 文件
cat /tmp/VIDEO_ID.*.vtt
```

### 获取评论

```bash
# 提取评论（best-effort，不保证完整）
yt-dlp --write-comments --skip-download --write-info-json \
  --extractor-args "youtube:max_comments=20" \
  -o "/tmp/%(id)s" "URL"
# 评论在 .info.json 的 comments 字段中
```

### 搜索视频

```bash
yt-dlp --dump-json "ytsearch5:query"
```

> **字幕注意**: 手动上传的字幕提取可靠；自动生成字幕可能存在行间重复，需后处理。
> **评论注意**: `--write-comments` 基于网页抓取（非 YouTube Data API），部分评论可能丢失。

### 无字幕兜底：Whisper 音频转写

```bash
# 视频没有字幕时的兜底：下载音频并用 Whisper 转写（Groq 免费 key 即可）
agent-reach transcribe "https://www.youtube.com/watch?v=VIDEO_ID"
agent-reach transcribe ./local_audio.mp3 -o /tmp/transcript.txt
```

> 需要先配置 key：`agent-reach configure groq-key gsk_xxx`（免费，console.groq.com）
> 或 `agent-reach configure openai-key sk-xxx`。默认 auto 模式：groq 失败自动降级 openai。

## B站 / Bilibili（bili-cli 为主，OpenCLI 补字幕）

> ⚠️ **不要用 yt-dlp 读 B站**：B站风控已全面 412 拦截 yt-dlp（实测最新版、直连/代理/带 Cookie 全部无效）。yt-dlp 只用于 YouTube。

### 视频详情/搜索/热门/排行 (bili-cli，只读无需登录)

```bash
# 视频详情（标题/UP主/时长/播放互动数据/字幕可用性）
bili video BVxxx

# 搜索视频
bili search "query" --type video -n 5

# 热门视频 / 排行榜
bili hot -n 10
bili rank -n 10

# 下载音频并切分为 ASR-ready WAV（无字幕时配合 agent-reach transcribe 转写）
bili audio BVxxx
```

### 字幕 (OpenCLI，需要桌面 Chrome)

```bash
# 字幕逐句带时间轴
opencli bilibili subtitle BVxxx

# OpenCLI 也能搜索/读视频元数据（备选）
opencli bilibili search "query" -f yaml
opencli bilibili video BVxxx -f yaml
```

### 零配置兜底：搜索 API 直连

```bash
UA="Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36"
curl -s -c /tmp/bili_ck.txt -o /dev/null -A "$UA" "https://www.bilibili.com/"
curl -s -b /tmp/bili_ck.txt -A "$UA" -e "https://www.bilibili.com/" \
  "https://api.bilibili.com/x/web-interface/search/all/v2?keyword=QUERY&page=1"
```

> **安装 bili-cli**: `pipx install bilibili-cli`（上游 2026-03 起停更但实测健康；只读场景无需登录，`bili login` 扫码可解锁动态/收藏等个人功能）。

## 小宇宙播客 / Xiaoyuzhou Podcast

### 转录单集播客（可选 --polish 增强标点）

```bash
# 输出 Markdown 文件到 /tmp/。--polish 让 Llama 3.3 70B 给文稿补中文标点+合理分段
~/.agent-reach/tools/xiaoyuzhou/transcribe.sh --polish "https://www.xiaoyuzhoufm.com/episode/EPISODE_ID"
```

> 转写 prompt 已要求 Whisper 输出中文标点；若标点效果仍不理想，可加 `--polish` 用 Groq 上免费的 Llama 3.3 70B 补标点+合理分段（9 分钟播客约多 ~7 秒）。每次转写多一轮 LLM 调用，按需使用。

### 前置要求

1. **ffmpeg**: `brew install ffmpeg`
2. **Groq API Key** (免费): https://console.groq.com/keys
3. **配置 Key**: `agent-reach configure groq-key YOUR_KEY`
4. **首次运行**: `agent-reach install --env=auto` 安装工具

### 检查状态

```bash
agent-reach doctor
```

> 输出 Markdown 文件默认保存到 `/tmp/`。

## 选择指南

| 场景 | 推荐工具 |
|-----|---------|
| YouTube 字幕 | yt-dlp |
| B站视频详情/搜索 | bili-cli |
| B站字幕 | opencli bilibili subtitle |
| 播客转录 | 小宇宙 transcribe.sh |
| 无字幕音视频 | agent-reach transcribe（B站音频先 `bili audio`） |
"##;

/// All reference files: (filename, content).
const REFERENCES: &[(&str, &str)] = &[
    ("search.md", REF_SEARCH),
    ("social.md", REF_SOCIAL),
    ("career.md", REF_CAREER),
    ("dev.md", REF_DEV),
    ("web.md", REF_WEB),
    ("video.md", REF_VIDEO),
];

// ── helpers ────────────────────────────────────────────────────────────────

/// Expand a path that may start with `~`.
fn expand_home(path: &str) -> PathBuf {
    if let Some(stripped) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped);
        }
    }
    if path.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            return home.join(&path[2..]);
        }
    }
    PathBuf::from(path)
}

/// Determine which locale is active — returns `true` for English.
fn is_english_locale() -> bool {
    let candidates = [
        env::var("AGENT_REACH_LANG").unwrap_or_default(),
        env::var("LC_ALL").unwrap_or_default(),
        env::var("LC_MESSAGES").unwrap_or_default(),
        env::var("LANG").unwrap_or_default(),
    ];
    for c in &candidates {
        let norm = c.trim().to_lowercase();
        if norm.starts_with("en") || norm.starts_with("english") {
            return true;
        }
    }
    false
}

/// Return the correct SKILL.md content based on locale.
fn skill_content() -> &'static str {
    if is_english_locale() {
        SKILL_CONTENT_EN
    } else {
        SKILL_CONTENT
    }
}

/// Build the list of skill directories to try, in priority order.
///
/// Priority: OPENCLAW_HOME (if set) > ~/.agents/skills > ~/.openclaw/skills > ~/.claude/skills
fn skill_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // OPENCLAW_HOME takes highest priority when set.
    if let Ok(openclaw_home) = env::var("OPENCLAW_HOME") {
        dirs.push(PathBuf::from(&openclaw_home).join(".openclaw").join("skills"));
    }

    // Standard directories.
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".agents").join("skills")); // generic agents (priority)
        dirs.push(home.join(".openclaw").join("skills")); // OpenClaw
        dirs.push(home.join(".claude").join("skills")); // Claude Code
    }

    dirs
}

/// Copy the entire skill directory (SKILL.md + references/) into `target`.
///
/// Returns `true` on success.
fn copy_skill_dir(target: &Path) -> bool {
    let result: Result<(), String> = (|| {
        // Clear existing installation. A symlinked skill dir can't use
        // remove_dir_all — unlink the link itself instead.
        if target.is_symlink() {
            fs::remove_file(target).map_err(|e| format!("unlink {}: {}", target.display(), e))?;
        } else if target.exists() {
            fs::remove_dir_all(target)
                .map_err(|e| format!("rmtree {}: {}", target.display(), e))?;
        }

        fs::create_dir_all(target)
            .map_err(|e| format!("mkdir {}: {}", target.display(), e))?;

        // Write SKILL.md.
        let skill_md = target.join("SKILL.md");
        fs::write(&skill_md, skill_content())
            .map_err(|e| format!("write {}: {}", skill_md.display(), e))?;

        // Write references/.
        let refs_dir = target.join("references");
        fs::create_dir_all(&refs_dir)
            .map_err(|e| format!("mkdir {}: {}", refs_dir.display(), e))?;

        for (name, content) in REFERENCES {
            let ref_path = refs_dir.join(name);
            fs::write(&ref_path, content)
                .map_err(|e| format!("write {}: {}", ref_path.display(), e))?;
        }

        Ok(())
    })();

    match result {
        Ok(()) => true,
        Err(e) => {
            eprintln!("  Warning: Could not install skill: {}", e);
            false
        }
    }
}

/// Return a human-readable platform name for a skill directory.
fn platform_name(dir: &Path) -> &str {
    let s = dir.to_string_lossy();
    if s.contains(".agents") {
        "Agent"
    } else if s.contains("openclaw") {
        "OpenClaw"
    } else if s.contains("claude") {
        "Claude Code"
    } else {
        "Agent"
    }
}

// ── public API ─────────────────────────────────────────────────────────────

/// Install the SKILL.md + references into every known agent skill directory.
///
/// Returns `Ok(())` when at least one directory was successfully populated, or
/// when a fallback `~/.agents/skills/agent-reach/` was created.
pub fn install_skill() -> Result<(), String> {
    let mut installed = false;

    for skill_dir in skill_dirs() {
        if skill_dir.is_dir() {
            let target = skill_dir.join("agent-reach");
            if copy_skill_dir(&target) {
                let name = platform_name(&skill_dir);
                println!("Skill installed for {}: {}", name, target.display());
                installed = true;
            }
        }
    }

    if !installed {
        // No known skill directory found — create for .agents by default.
        let fallback = expand_home("~/.agents/skills/agent-reach");
        if let Some(parent) = fallback.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                format!("Could not create skill directory {}: {}", parent.display(), e)
            })?;
        }
        if copy_skill_dir(&fallback) {
            println!("Skill installed: {}", fallback.display());
        } else {
            eprintln!("  -- Could not install agent skill (optional)");
            eprintln!(
                "  -- Tip: install OpenClaw, Claude Code, or create ~/.agents/skills/ manually"
            );
        }
    }

    Ok(())
}

/// Remove the SKILL.md + references from all known agent skill directories.
///
/// Returns `Ok(())` regardless of whether anything was found (no error if
/// nothing to remove).
pub fn uninstall_skill() -> Result<(), String> {
    // (platform path template, platform name), in removal order.
    let mut entries: Vec<(PathBuf, &str)> = Vec::new();

    // OPENCLAW_HOME takes priority.
    if let Ok(openclaw_home) = env::var("OPENCLAW_HOME") {
        entries.push((
            PathBuf::from(&openclaw_home)
                .join(".openclaw")
                .join("skills")
                .join("agent-reach"),
            "OpenClaw",
        ));
    }

    if let Some(home) = dirs::home_dir() {
        entries.push((home.join(".openclaw").join("skills").join("agent-reach"), "OpenClaw"));
        entries.push((home.join(".claude").join("skills").join("agent-reach"), "Claude Code"));
        entries.push((home.join(".agents").join("skills").join("agent-reach"), "Agent"));
    }

    let mut removed = false;
    for (path, name) in &entries {
        if path.is_dir() {
            let res: io::Result<()> = if path.is_symlink() {
                fs::remove_file(path)
            } else {
                fs::remove_dir_all(path)
            };
            match res {
                Ok(()) => {
                    println!("  Removed {} skill: {}", name, path.display());
                    removed = true;
                }
                Err(e) => {
                    eprintln!("  Could not remove {}: {}", path.display(), e);
                }
            }
        }
    }

    if !removed {
        println!("  No skill installations found.");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_content_is_nonempty() {
        assert!(!SKILL_CONTENT.is_empty());
        assert!(!SKILL_CONTENT_EN.is_empty());
    }

    #[test]
    fn test_references_complete() {
        assert_eq!(REFERENCES.len(), 6);
        for (name, content) in REFERENCES {
            assert!(!content.is_empty(), "reference {} is empty", name);
            assert!(name.ends_with(".md"), "reference {} not .md", name);
        }
    }

    #[test]
    fn test_is_english_locale_defaults_false() {
        // Without env vars set, should return false (Chinese default).
        // Cannot easily isolate env in unit tests, but we can at least
        // call it and ensure it doesn't panic.
        let _ = is_english_locale();
    }

    #[test]
    fn test_platform_name() {
        let agents = std::path::Path::new("/home/user/.agents/skills");
        let openclaw = std::path::Path::new("/home/user/.openclaw/skills");
        let claude = std::path::Path::new("/home/user/.claude/skills");

        assert_eq!(platform_name(agents), "Agent");
        assert_eq!(platform_name(openclaw), "OpenClaw");
        assert_eq!(platform_name(claude), "Claude Code");
    }

    #[test]
    fn test_skill_dirs_nonempty() {
        let dirs = skill_dirs();
        // Should always have at least the standard dirs (when home_dir() works).
        assert!(!dirs.is_empty(), "skill_dirs() returned empty list");
    }
}
