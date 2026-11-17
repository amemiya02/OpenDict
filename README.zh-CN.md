# OpenDict

[English](README.md) | **简体中文**

OpenDict 是一个开源、跨平台的词典客户端，用来阅读你已有的 MDict 词典文件（`.mdx` / `.mdd`）。目标是在功能上全面对标 MDict，界面完全重新设计：一次查询里所有词典在同一页作答，各自保留原有排版，所有操作都能用键盘完成。

> **当前状态：尚未发布。** v0.1 里程碑（见 [`docs/roadmap.md`](docs/roadmap.md)）的功能已经完成，但还没有发布任何版本；发布后的安装方式见“安装”一节。

## 仓库内容

| 路径 | 内容 |
|---|---|
| [`PRODUCT.md`](PRODUCT.md) | 产品为谁而做、必须做到什么 |
| [`GLOSSARY.md`](GLOSSARY.md) | 项目术语表 |
| [`docs/roadmap.md`](docs/roadmap.md) | 立项决定、架构和里程碑 |
| [`docs/adr/`](docs/adr/) | 架构决策记录 |
| [`DESIGN.md`](DESIGN.md) | 设计系统（「辞书版面」） |
| [`design/prototype/`](design/prototype/) | 主窗口和迷你窗口的可点击 HTML 原型 |
| [`crates/mdict/`](crates/mdict/) | `opendict-mdict`，MDict 解析库（MIT 或 Apache-2.0） |
| [`crates/opendict-core/`](crates/opendict-core/) | `opendict-core`：词库与查词，不含界面 |
| [`crates/mdict-fixture/`](crates/mdict-fixture/) | 生成测试用的小型 MDX 文件 |
| [`apps/desktop/`](apps/desktop/) | 桌面应用：Tauri 2 外壳和 React 界面 |

试用原型：在仓库根目录启动一个 HTTP 服务，再打开 `/design/prototype/`：

```bash
python -m http.server 8000
```

然后访问 <http://localhost:8000/design/prototype/>。原型里的词条都是手写的示例，并非来自真实词典。

## 安装

每个 [GitHub Release](https://github.com/amemiya02/OpenDict/releases) 发布后都附有 Windows、macOS 和 Linux 的安装包。已安装的 OpenDict 启动时会检查是否有新版本，在状态栏中提示，点击 **安装并重新启动** 即可一步完成更新。

OpenDict 没有使用付费证书进行代码签名，所以 Windows 和 macOS 在第一次打开时会给出警告。更新包本身用 OpenDict 自己的密钥签名，安装前会先校验。

- **Windows。** 运行 `OpenDict_<版本>_x64-setup.exe`。如果 SmartScreen 提示“Windows 已保护你的电脑”，点击 **更多信息**，再点击 **仍要运行**。
- **macOS。** 打开与你的 Mac 对应的 `.dmg`（Apple 芯片选 `aarch64`，Intel 选 `x64`），把 OpenDict 拖进“应用程序”。第一次打开时 macOS 会提示无法验证开发者：点击 **完成**，然后打开 **系统设置 → 隐私与安全性**，向下滚动，在关于 OpenDict 的提示旁点击 **仍要打开**。也可以在终端里运行一次 `xattr -dr com.apple.quarantine /Applications/OpenDict.app`。
- **Linux。** 使用 `.AppImage`（先 `chmod +x` 赋予执行权限再运行），或用 `sudo apt install ./OpenDict_<版本>_amd64.deb` 安装 `.deb`。只有 AppImage 能自动更新；使用 `.deb` 时，每个新版本都按同样方式安装。

维护者：发布和签名流程见 [`docs/releasing.md`](docs/releasing.md)。

## 开发

需要 Rust（stable）、Node.js 24 或更新版本，以及所用平台的 [Tauri 前置依赖](https://v2.tauri.app/start/prerequisites/)。

```bash
cargo test --workspace                       # 解析库、核心和外壳的测试
cargo run -p opendict-mdict-fixture --example sample_library -- sample-library
cd apps/desktop
npm ci
npm run typecheck && npm run lint && npm test
OPENDICT_LIBRARY_FOLDER=../../sample-library npx tauri dev
```

应用从配置目录下 `config.json` 的 `libraryFolder` 读取词库文件夹，也可以用环境变量 `OPENDICT_LIBRARY_FOLDER` 指定。在 Windows 上，`npx tauri build --debug --no-bundle && npm run e2e` 会对构建好的应用运行端到端冒烟测试。

## 计划

- **桌面端。** Windows、macOS、Linux，基于 Tauri 和 Rust；之后可能会有 Android 版。
- **读取 MDict 文件。** 支持 MDX/MDD 1.2 与 2.0 版本、常见压缩和编码，以及两种 MDict 加密。
- **查词。** 在一组词典里输入即查，支持通配符、模糊匹配，以及英语和日语的词形还原。
- **阅读工具。** 历史记录、可导出 CSV/TSV 的生词本，以及可在任意应用里查选中文字的迷你窗口。
- **界面语言。** 简体中文、英文、日文。
- **隐私。** 纯本地运行，无账号、无服务器。

完整计划见 [`docs/roadmap.md`](docs/roadmap.md)。

## 许可证

应用以 [GNU GPL v3.0 或更新版本](LICENSE) 发布。MDict 解析库（`crates/mdict`）以 MIT 或 Apache-2.0 双许可发布，方便其他项目复用。详见 [ADR-0001](docs/adr/0001-gpl-app-permissive-parser.md)。

OpenDict 不附带任何词典，请使用你有权使用的词典。
