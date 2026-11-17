# OpenDict

**English** | [简体中文](#简体中文)

OpenDict is an open-source, cross-platform dictionary reader for the MDict dictionary files (`.mdx` / `.mdd`) you already own. It aims to match MDict feature for feature with a completely rebuilt interface: every dictionary answers on one page, each in its own typographic style, and every action is reachable from the keyboard.

> **Status: pre-alpha.** No application code exists yet. This repository holds the founding documents and a clickable design prototype. Nothing is ready to install.

## What is here

| Path | What it is |
|---|---|
| [`PRODUCT.md`](PRODUCT.md) | Who OpenDict is for and what it must do |
| [`GLOSSARY.md`](GLOSSARY.md) | The project's vocabulary |
| [`docs/roadmap.md`](docs/roadmap.md) | Founding decisions, architecture and milestones |
| [`docs/adr/`](docs/adr/) | Architecture decision records |
| [`DESIGN.md`](DESIGN.md) | The design system ("The Lexicographer's Page") |
| [`design/prototype/`](design/prototype/) | Clickable HTML prototype of the main window and the Mini Window |

To try the prototype, serve the repository root over HTTP and open `/design/prototype/`:

```bash
python -m http.server 8000
```

Then open <http://localhost:8000/design/prototype/>. Every entry in the prototype is a written sample, not taken from a real dictionary.

## Planned

- **Desktop.** Windows, macOS and Linux, built on Tauri and Rust. Android may follow later.
- **Reading MDict files.** MDX/MDD versions 1.2 and 2.0, common compressions and encodings, and both kinds of MDict encryption.
- **Lookup.** Lookup as you type across a Group of dictionaries, with wildcards, fuzzy matching, and inflection handling for English and Japanese.
- **Reading tools.** History, Word Books with CSV/TSV export, and a Mini Window that looks up selected text from any application.
- **Interface languages.** Simplified Chinese, English and Japanese.
- **Privacy.** Local only: no account and no server.

See [`docs/roadmap.md`](docs/roadmap.md) for the full plan.

## License

The application is licensed under the [GNU GPL v3.0 or later](LICENSE). The MDict parsing library (`crates/mdict`, not yet written) will be licensed MIT OR Apache-2.0 so other projects can reuse it. See [ADR-0001](docs/adr/0001-gpl-app-permissive-parser.md).

OpenDict ships no dictionaries. Use dictionaries you have the right to use.

---

## 简体中文

OpenDict 是一个开源、跨平台的词典客户端，用来阅读你已有的 MDict 词典文件（`.mdx` / `.mdd`）。目标是在功能上全面对标 MDict，界面完全重新设计：一次查询里所有词典在同一页作答，各自保留原有排版，所有操作都能用键盘完成。

> **当前状态：预览前阶段。** 还没有应用代码，本仓库目前只有项目文档和一个可点击的设计原型，暂无可安装的版本。

### 仓库内容

| 路径 | 内容 |
|---|---|
| [`PRODUCT.md`](PRODUCT.md) | 产品为谁而做、必须做到什么 |
| [`GLOSSARY.md`](GLOSSARY.md) | 项目术语表 |
| [`docs/roadmap.md`](docs/roadmap.md) | 立项决定、架构和里程碑 |
| [`docs/adr/`](docs/adr/) | 架构决策记录 |
| [`DESIGN.md`](DESIGN.md) | 设计系统（「辞书版面」） |
| [`design/prototype/`](design/prototype/) | 主窗口和迷你窗口的可点击 HTML 原型 |

试用原型：在仓库根目录启动一个 HTTP 服务，再打开 `/design/prototype/`：

```bash
python -m http.server 8000
```

然后访问 <http://localhost:8000/design/prototype/>。原型里的词条都是手写的示例，并非来自真实词典。

### 计划

- **桌面端。** Windows、macOS、Linux，基于 Tauri 和 Rust；之后可能会有 Android 版。
- **读取 MDict 文件。** 支持 MDX/MDD 1.2 与 2.0 版本、常见压缩和编码，以及两种 MDict 加密。
- **查词。** 在一组词典里输入即查，支持通配符、模糊匹配，以及英语和日语的词形还原。
- **阅读工具。** 历史记录、可导出 CSV/TSV 的生词本，以及可在任意应用里查选中文字的迷你窗口。
- **界面语言。** 简体中文、英文、日文。
- **隐私。** 纯本地运行，无账号、无服务器。

完整计划见 [`docs/roadmap.md`](docs/roadmap.md)。

### 许可证

应用以 [GNU GPL v3.0 或更新版本](LICENSE) 发布。MDict 解析库（`crates/mdict`，尚未编写）将以 MIT 或 Apache-2.0 双许可发布，方便其他项目复用。详见 [ADR-0001](docs/adr/0001-gpl-app-permissive-parser.md)。

OpenDict 不附带任何词典，请使用你有权使用的词典。
