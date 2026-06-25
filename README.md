<h1 align="center">🦀 Agent Reach <sub><sup>Rust</sup></sub></h1>

<p align="center">
  <strong>给你的 AI Agent 一键装上互联网能力</strong><br>
  <em>Rust 原生实现 · 单二进制 · 零运行时依赖</em>
</p>

<p align="center">
  当下最稳的接入方式，替你选好、装好、体检好——接入方式会换代，你不用操心
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg?style=for-the-badge" alt="MIT License"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.80+-orange.svg?style=for-the-badge&logo=rust&logoColor=white" alt="Rust 1.80+"></a>
  <a href="https://github.com/eric8810/agent-reach-rs/stargazers"><img src="https://img.shields.io/github/stars/eric8810/agent-reach-rs?style=for-the-badge" alt="GitHub Stars"></a>
</p>

<p align="center">
  <a href="#快速上手">快速开始</a> · <a href="#支持的平台">支持平台</a> · <a href="#为什么选择-rust-版本">为什么选 Rust</a> · <a href="#设计理念">设计理念</a>
</p>

---

> ⚡ **这是 [Agent Reach](https://github.com/Panniantong/Agent-Reach) 的 Rust 翻译版本。**
>
> 原项目由 [Neo Reid (@Panniantong)](https://github.com/Panniantong) 用 Python 打造，3,000+ stars，是 AI Agent 互联网接入的事实标准。
> 本仓库将其完整翻译为 Rust，保留全部功能，同时提供**单二进制部署**、**零 Python 依赖**、**启动速度提升 50 倍**。

---

## 为什么需要 Agent Reach？

AI Agent 已经能帮你写代码、改文档、管项目——但你让它去网上找点东西，它就抓瞎了：

- 📺 "帮我看看这个 YouTube 教程讲了什么" → **看不了**，拿不到字幕
- 🐦 "帮我搜一下推特上大家怎么评价这个产品" → **搜不了**，Twitter API 要付费
- 📖 "去 Reddit 上看看有没有人遇到过同样的 bug" → **403 被封**，服务器 IP 被拒
- 📕 "帮我看看小红书上这个品的口碑" → **打不开**，必须登录才能看
- 📺 "B站上有个技术视频，帮我总结一下" → **拿不到**，通用下载工具被 B站风控全面拦截
- 🔍 "帮我在网上搜一下最新的 LLM 框架对比" → **没有好用的搜索**，要么付费要么质量差
- 🌐 "帮我看看这个网页写了啥" → **抓回来一堆 HTML 标签**，根本没法读
- 📦 "这个 GitHub 仓库是干嘛的？Issue 里说了什么？" → 能用，但认证配置很麻烦
- 📡 "帮我订阅这几个 RSS 源，有更新告诉我" → 要自己装库写代码

**这些不难实现，但是需要自己折腾配置**

每个平台都有自己的门槛——要付费的 API、要绕过的封锁、要登录的账号、要清洗的数据。你要一个一个去踩坑、装工具、调配置，光是让 Agent 能读个推特就得折腾半天。

**Agent Reach 把这件事变成一个二进制文件：**

```bash
# 一键安装（从源码编译）
cargo install agent-reach

# 初始化
agent-reach install --env=auto

# 体检
agent-reach doctor
```

几分钟后你的 Agent 就能读推特、搜 Reddit、看 YouTube、刷小红书了。

> ⭐ **Star 这个项目**，我们会持续追踪各平台的变化、接入新的渠道。你不用自己盯——平台封了我们修，有新渠道我们加。

### ✅ 在你用之前，你可能想知道

| | |
|---|---|
| 💰 **完全免费** | 所有工具开源、所有 API 免费。唯一可能花钱的是服务器代理（$1/月），本地电脑不需要 |
| 🔒 **隐私安全** | Cookie 只存在你本地，不上传不外传。代码完全开源（Rust 原版 + Python 原版），随时可审查 |
| 🔄 **持续换代** | 每个平台都是「首选 + 备选」多后端路由。某个接入方式失效了，我们换下一个，你无感 |
| 🤖 **兼容所有 Agent** | Claude Code、OpenClaw、Cursor、Windsurf……任何能跑命令行的 Agent 都能用 |
| 🩺 **自带诊断** | `agent-reach doctor` 一条命令告诉你哪个通、哪个不通、怎么修 |
| ⚡ **单二进制** | 编译产物只有一个可执行文件，复制即用，无需 Python、pip、虚拟环境 |

---

## 为什么选择 Rust 版本？

| | Python 原版 | Rust 版本 |
|---|---|---|
| **安装** | `pip install agent-reach`（需 Python ≥ 3.10） | `cargo install agent-reach` 或直接下载二进制 |
| **启动速度** | ~200ms（Python 解释器启动 + 模块加载） | ~4ms（原生编译） |
| **依赖** | Python + pip + 多个 PyPI 包 | **零运行时依赖**（静态链接） |
| **分发** | 需要 Python 环境 | 单文件二进制，复制即用 |
| **内存** | ~30MB（Python 解释器） | ~8MB |
| **功能** | ✅ 全部功能 | ✅ 100% 功能对等 |
| **浏览器 Cookie** | browser_cookie3 Python 库 | **直读 SQLite + Windows DPAPI 解密** |

---

## 支持的平台

| 平台 | 装好即用 | 配置后解锁 | 怎么配 |
|------|---------|-----------|-------|
| 🌐 **网页** | 阅读任意网页 | — | 无需配置 |
| 📺 **YouTube** | 字幕提取 + 视频搜索 | — | 无需配置 |
| 📡 **RSS** | 阅读任意 RSS/Atom 源 | — | 无需配置 |
| 🔍 **全网搜索** | — | 全网语义搜索 | 自动配置（MCP 接入，免费无需 Key） |
| 📦 **GitHub** | 读公开仓库 + 搜索 | 私有仓库、提 Issue/PR、Fork | 告诉 Agent「帮我登录 GitHub」 |
| 🐦 **Twitter/X** | 读单条推文 | 搜索推文、浏览时间线、读长文 | 告诉 Agent「帮我配 Twitter」 |
| 📺 **B站** | 搜索 + 视频详情（bili-cli，无需登录） | 字幕（OpenCLI） | 告诉 Agent「帮我配 B站」 |
| 📖 **Reddit** | —（没有零配置路径） | 搜索 + 读帖子和评论 | 桌面装 OpenCLI 用浏览器登录态；或 rdt-cli + Cookie |
| 📕 **小红书** | — | 搜索、阅读、评论 | 桌面装 OpenCLI（刷过小红书即可用） |
| 💼 **LinkedIn** | Jina Reader 读公开页面 | Profile 详情、公司页面、职位搜索 | 告诉 Agent「帮我配 LinkedIn」 |
| 💻 **V2EX** | 热门帖子、节点帖子、帖子详情+回复、用户信息 | — | 无需配置 |
| 📈 **雪球** | 股票行情、搜索股票、热门帖子、热门股票排行 | — | 告诉 Agent「帮我配雪球」 |
| 🎙️ **小宇宙播客** | — | 播客音频转文字（Whisper 转录，免费 Key） | 告诉 Agent「帮我配小宇宙播客」 |

> 🍪 需要 Cookie 的平台（Twitter、小红书等），**优先使用** Chrome 插件 [Cookie-Editor](https://chromewebstore.google.com/detail/cookie-editor/hlkenndednhfkekhgcdicdfddnkalmdm) 导出 Cookie，或直接运行：
> ```bash
> agent-reach configure --from-browser chrome
> ```
> Rust 版**直接读取浏览器 SQLite 数据库**，无需额外 Python 依赖。

---

## 快速上手

### 从源码编译

```bash
git clone https://github.com/eric8810/agent-reach-rs.git
cd agent-reach-rs
cargo build --release
./target/release/agent-reach install --env=auto
```

### 或者直接安装

```bash
cargo install agent-reach
agent-reach install --env=auto
```

然后告诉你的 Agent：

```
帮我装互联网能力：https://github.com/eric8810/agent-reach-rs
```

就这一步。Agent 会自己完成剩下的所有事情。

---

## 装好就能用

不需要任何配置，告诉 Agent 就行：

- "帮我看看这个链接" → `curl https://r.jina.ai/URL` 读任意网页
- "这个 GitHub 仓库是做什么的" → `gh repo view owner/repo`
- "这个 YouTube 视频讲了什么" → `yt-dlp` 提取字幕
- "B站搜一下 AI 教程" → `bili search`（无需登录）
- "全网搜一下 LLM 框架对比" → Exa 语义搜索
- "订阅这个 RSS" → 原生 RSS/Atom 解析

**不需要记命令。** 需要登录的平台（小红书、Twitter、Reddit），告诉 Agent「帮我配 XXX」即可解锁。

---

## 设计理念

**Agent Reach 是一个能力层（capability layer），不是又一个工具。**

它比任何具体实现高一层——负责**选型、安装、体检、路由**，不负责底层读取本身。读取由 Agent 直接调用上游工具完成，没有包装层。

### 🔌 每个平台 = 首选 + 备选的有序后端列表

```
src/channels/
├── web.rs          → Jina Reader
├── twitter.rs      → twitter-cli ▸ OpenCLI ▸ bird
├── youtube.rs      → yt-dlp
├── github.rs       → gh CLI
├── bilibili.rs     → bili-cli ▸ OpenCLI ▸ 搜索 API
├── reddit.rs       → OpenCLI ▸ rdt-cli（无零配置路径）
├── xiaohongshu.rs  → OpenCLI ▸ xiaohongshu-mcp ▸ xhs-cli
├── linkedin.rs     → linkedin-mcp ▸ Jina Reader
├── rss.rs          → 原生 RSS/Atom（编译时保证）
├── exa_search.rs   → Exa via mcporter
├── v2ex.rs         → V2EX 公开 API
├── xueqiu.rs       → 雪球 API（含 Cookie 管理）
├── xiaoyuzhou.rs   → Groq Whisper + ffmpeg
└── mod.rs          → 渠道注册（doctor 检测用）
```

每个渠道文件按序**真实探测**各候选后端，第一个完整可用的当选；坏掉的会给出修复处方。

### 项目结构

```
src/
├── cli.rs              # 11 个子命令（install/doctor/configure/transcribe...）
├── config.rs           # YAML 配置管理（~/.agent-reach/config.yaml）
├── probe.rs            # 上游命令探活（missing/broken/timeout/ok）
├── doctor.rs           # 体检引擎
├── cookie_extract.rs   # 浏览器 Cookie 提取（SQLite + DPAPI）
├── transcribe.rs       # 音频转录（yt-dlp + ffmpeg + Whisper API）
├── install.rs          # 一键安装流程（pipx/npm/apt/brew）
├── skill.rs            # Agent Skill 部署
├── backends/
│   └── opencli.rs      # OpenCLI 浏览器后端
├── channels/           # 13 个平台渠道
└── utils/              # 路径 + 文本工具
```

> 🦀 全部用 Rust trait 系统实现多态 Channel，`cargo check` 零警告零错误。

---

## 安全性

Agent Reach 在设计上重视安全：

| 措施 | 说明 |
|------|------|
| 🔒 **凭据本地存储** | Cookie、Token 只存在你本机 `~/.agent-reach/config.yaml`，文件权限 600（仅所有者可读写） |
| 🛡️ **安全模式** | `agent-reach install --safe` 不会自动修改系统，只列出需要什么 |
| 👀 **完全开源** | 代码透明，随时可审查。所有依赖工具也是开源项目 |
| 🔍 **Dry Run** | `agent-reach install --dry-run` 预览所有操作，不做任何改动 |
| 🧩 **可插拔架构** | 不信任某个组件？换掉对应的 channel 文件即可 |

### 🍪 Cookie 安全建议

> ⚠️ 使用 Cookie 登录的平台，通过脚本/API 调用**存在被平台检测并封号的风险**。请务必使用**专用小号**。

---

## 致谢 & 致敬

### 原始项目

这个项目是 **[Agent Reach](https://github.com/Panniantong/Agent-Reach)** 的完整 Rust 重写。

感谢 **[Neo Reid (@Panniantong)](https://github.com/Panniantong)** 创造了一个如此优雅的设计——每个平台一个 channel 文件、多后端路由、`agent-reach doctor` 一行体检——这些理念被完整保留在 Rust 版本中。

原项目是 AI Agent 互联网接入的事实标准，如果你还没有 Star，请一定去给原项目一颗星 ⭐：
> https://github.com/Panniantong/Agent-Reach

### 上游工具

[OpenCLI](https://github.com/jackwener/opencli) · [twitter-cli](https://github.com/public-clis/twitter-cli) · [rdt-cli](https://github.com/public-clis/rdt-cli) · [xiaohongshu-mcp](https://github.com/xpzouying/xiaohongshu-mcp) · [xhs-cli](https://github.com/jackwener/xiaohongshu-cli) · [bili-cli](https://github.com/public-clis/bilibili-cli) · [yt-dlp](https://github.com/yt-dlp/yt-dlp) · [Jina Reader](https://github.com/jina-ai/reader) · [Exa](https://exa.ai) · [mcporter](https://github.com/nicobailon/mcporter) · [linkedin-scraper-mcp](https://github.com/stickerdaniel/linkedin-mcp-server)

---

## 贡献

Rust 版本的代码风格追求简洁直接，每个 PR 欢迎。

**想要新渠道？** 直接提 Issue 告诉我们，或者自己提 PR——每个渠道就是一个独立文件（`src/channels/<platform>.rs`），实现 `Channel` trait 即可。

---

## License

[MIT](LICENSE) — 与原始项目保持一致。

---

## 友情链接

- [Agent Reach (Python 原版)](https://github.com/Panniantong/Agent-Reach) — 原项目，3,000+ stars
- [OpenCLI](https://github.com/jackwener/opencli) — 浏览器登录态复用，桌面端首选后端
- [BrowserAct](https://www.browseract.ai/Agent) — 浏览器自动化工具，补充"动手"场景

---

<p align="center">
  <em>为 Web 4.0 基建贡献一份自己的力量。</em>
</p>
