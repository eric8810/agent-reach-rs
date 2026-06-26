# 🦀 Rust-Native Roadmap

> 目标：消除所有外部 CLI 依赖，让 Agent Reach 成为真正的单二进制工具。
> 当前 22 个 Rust crate 依赖 + 13 个外部 CLI 工具 → 目标 24 个 Rust crate 依赖 + 0 个外部工具。

---

## 总览

| # | Issue | 外部工具 | 替代方案 | 代码量 | 优先级 | 难度 |
|---|-------|---------|---------|--------|--------|------|
| 1 | Twitter GraphQL | `twitter-cli` | `ureq` + cookie → Twitter GraphQL API | ~400 行 | 🔴 P0 | ⭐⭐ |
| 2 | YouTube 字幕 | `yt-dlp` + `node` | `rusty_ytdl` crate + InnerTube API | ~100 行 | 🔴 P0 | ⭐ |
| 3 | Reddit 读取 | `rdt-cli` | `ureq` + cookie → Reddit `.json` API | ~250 行 | 🔴 P0 | ⭐ |
| 4 | GitHub 仓库 | `gh` | `ureq` + token → `api.github.com` | ~200 行 | 🟡 P1 | ⭐ |
| 5 | B站搜索/详情 | `bili-cli` | 扩展已有 API 回退 → 完整 B站 API | ~300 行 | 🟡 P1 | ⭐⭐ |
| 6 | ffmpeg 音频 | `ffmpeg` | `ffmpeg-next` crate（FFI 绑定） | ~50 行 | 🟡 P1 | ⭐ |
| 7 | mcporter MCP | `mcporter` | **已有** `mcp_server.rs`，需补 client 端 | ~100 行 | 🟢 P2 | ⭐⭐ |
| 8 | 小红书 | `xhs-cli` / `opencli` | `ureq` + cookie → 小红书 API | ~500+ 行 | 🟢 P2 | ⭐⭐⭐ |
| 9 | 安装器精简 | `pipx`/`npm`/`brew` | 不再需要装外部工具，删除安装逻辑 | ~50 行 | 🟢 P2 | ⭐ |

**完成后的状态：**
- 外部 CLI 依赖：13 → 0
- Rust crate 依赖：22 → ~24（+`rusty_ytdl`、+`ffmpeg-next`、+`aes`）
- 新增代码：~1,800 行
- 二进制大小：~5MB → ~8MB（静态链接 ffmpeg）

---

## Issue 1: Twitter/X 原生支持

**当前：** `twitter-cli`（Python，pipx 安装）  
**目标：** 直调 Twitter GraphQL API，零外部依赖  

**技术方案：**
- Twitter 内网 GraphQL API（`x.com/i/api/graphql/...`）
- Cookie 认证（`auth_token` + `ct0`）
- 端点：`SearchTimeline`（搜索）、`TweetDetail`（读推文）、`UserByScreenName`（用户信息）
- csrf token 从 Cookie 中 `ct0` 字段获取
- 请求头需要 `x-twitter-active-user`、`x-twitter-client-language` 等
- 已在 `cookie_extract.rs` 中支持 Cookie 提取

**代码量：** ~400 行  
**参考：** [twitter-cli](https://github.com/public-clis/twitter-cli) 源码、[Twitter GraphQL 文档](https://github.com/fa0311/twitter-api)

---

## Issue 2: YouTube 字幕原生支持

**当前：** `yt-dlp`（Python）+ `node`/`deno`（JS runtime 签名解密）  
**目标：** 调 `rusty_ytdl` crate，零外部依赖  

**技术方案：**
- [`rusty_ytdl`](https://crates.io/crates/rusty_ytdl) crate：纯 Rust YouTube 下载器
- 支持视频信息提取 + 字幕下载
- 不需要 JS runtime（签名解密已内置）
- 已支持多种字幕格式（vtt、srt、json3）

**代码量：** ~100 行  
**Cargo.toml：** `rusty_ytdl = "0.7"`

---

## Issue 3: Reddit 原生支持

**当前：** `rdt-cli`（Python，pipx 安装）或 `opencli`（Chrome 扩展）  
**目标：** 直调 Reddit JSON API，零外部依赖  

**技术方案：**
- Reddit 的 `.json` 后缀 API（如 `reddit.com/r/rust.json`）
- Cookie 认证（`reddit_session`）
- 搜索：`reddit.com/search.json?q=xxx&type=link`
- 评论：`reddit.com/comments/{post_id}.json`
- Subreddit：`reddit.com/r/{subreddit}/hot.json`
- 需要 OAuth `User-Agent` 头（格式：`platform:app_id:version (by /u/username)`）

**代码量：** ~250 行  
**注意事项：** Reddit 对非登录态请求返回 403，必须有 Cookie

---

## Issue 4: GitHub 原生支持

**当前：** `gh` CLI（Go 二进制，brew/apt 安装）  
**目标：** 直调 GitHub REST API，零外部依赖  

**技术方案：**
- GitHub REST API v3：`api.github.com`
- Personal Access Token 认证（`Authorization: Bearer xxx`）
- 仓库信息：`GET /repos/{owner}/{repo}`
- Issue 搜索：`GET /search/issues?q=xxx`
- 代码搜索：`GET /search/code?q=xxx`
- 已在 `config.rs` 中支持 `github_token`

**代码量：** ~200 行  

---

## Issue 5: B站完整 API

**当前：** `bili-cli`（Python，pipx 安装），已有搜索 API 回退  
**目标：** 扩展已有 `_check_search_api` → 完整 B站功能  

**技术方案：**
- **已有**：搜索 API（`api.bilibili.com/x/web-interface/search`）
- **需补**：视频详情（`api.bilibili.com/x/web-interface/view?bvid=xxx`）
- **需补**：热门排行（`api.bilibili.com/x/web-interface/ranking/v2`）
- **需补**：用户投稿（`api.bilibili.com/x/space/wbi/arc/search`）
- 无需登录即可调用（公开 API）
- WBI 签名（w_rid + wts，已有社区实现）

**代码量：** ~300 行  

---

## Issue 6: ffmpeg → ffmpeg-next

**当前：** `ffmpeg`（C 二进制，brew/apt 安装）  
**目标：** 通过 `ffmpeg-next` crate 静态链接或调用系统 ffmpeg  

**技术方案：**
- [`ffmpeg-next`](https://crates.io/crates/ffmpeg-next) crate：Rust FFI 绑定
- 音频转码：`-vn -ac 1 -ar 16000 -b:a 32k`
- 音频切片：`-f segment -segment_time 600`
- 用于 `transcribe.rs` 和 `xiaoyuzhou` 渠道

**代码量：** ~50 行（替换现有 `std::process::Command` 调用）  
**Cargo.toml：** `ffmpeg-next = "7"`

---

## Issue 7: mcporter → 内建 MCP 路由

**当前：** `mcporter`（Node.js，npm 安装）用于 Exa/LinkedIn/小红书-mcp  
**目标：** 已有 `mcp_server.rs`，补 MCP client 端直连  

**技术方案：**
- MCP 协议 = JSON-RPC over stdio/HTTP
- Exa MCP 端点：`https://mcp.exa.ai/mcp`（HTTP SSE）
- linkedin-scraper-mcp：`http://localhost:3000/mcp`
- xiaohongshu-mcp：`http://localhost:18060/mcp`
- 用 `ureq` 直接调 MCP 端点，不需要 mcporter 中转

**代码量：** ~100 行  

---

## Issue 8: 小红书原生支持

**当前：** `xhs-cli`（Python，已停更）/ `opencli` / `xiaohongshu-mcp`  
**目标：** 直调小红书 API  

**技术方案：**
- 小红书 API 有强反爬（验证码、签名、设备指纹）
- 桌面端：复用浏览器 Cookie（通过 `opencli` 或直接读浏览器）
- 服务器端：xiaohongshu-mcp（自带无头浏览器）
- 建议方向：用 `ureq` + Cookie → 搜索/笔记详情 API
- xs、xt 签名算法需要逆向（社区有部分实现）

**代码量：** ~500+ 行  
**难度：⭐⭐⭐** — 反爬是持续对抗，可能需要定期更新

---

## Issue 9: 安装器精简

**当前：** `install.rs` 860 行，依赖 `pipx`/`npm`/`brew`/`apt-get`  
**目标：** 删除不再需要的安装逻辑  

**删除项：**
- `install_twitter_deps()` → Twitter 已内置
- `install_bili_deps()` → B站已内置  
- `install_rdt_cli()` → Reddit 已内置
- `install_opencli_deps()` → 如果小红书也内置，可删
- `install_mcporter()` → mcporter 已替代
- `install_system_deps()` → 不再需要装 gh/node/ffmpeg

**保留项：**
- `install_skill()` → 仍然需要部署 SKILL.md
- 环境检测 → 仍有参考价值
- 或者整个 `install` 命令简化为 `doctor` + `configure` + `skill`

**代码量：** ~50 行（删除多于新增）

---

## 实施顺序建议

```
第一阶段（核心渠道，~750 行）：
  1. YouTube (rusty_ytdl)     ← 最简单，调 crate
  2. Reddit (ureq + cookie)    ← 简单 HTTP API
  3. Twitter (ureq + cookie)   ← 中等，GraphQL 需要文档

第二阶段（扩展渠道，~500 行）：
  4. GitHub (ureq + token)     ← 标准 REST API
  5. B站 (扩展现有代码)         ← 公开 API，无认证

第三阶段（基础设施，~200 行）：
  6. ffmpeg-next              ← 替换外部二进制
  7. mcporter → 内建 MCP       ← 已有基础

第四阶段（困难，~500+ 行）：
  8. 小红书 (ureq + cookie)    ← 需要逆向签名
  9. 安装器精简
```
