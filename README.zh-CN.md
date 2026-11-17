# OpenDict

[English](README.md) | **简体中文**

开源、跨平台的 MDict 词典（`.mdx` / `.mdd`）阅读器。一次查询，所有词典在同一页作答，各自保留原有排版，所有操作都能用键盘完成。

## 安装

从 [Releases](https://github.com/amemiya02/OpenDict/releases) 下载对应系统的安装包，之后 OpenDict 会自动更新。

OpenDict 没有代码签名，首次打开时系统会警告：

- **Windows：** 在 SmartScreen 中点击 **更多信息 → 仍要运行**。
- **macOS：** 打开 **系统设置 → 隐私与安全性**，点击 **仍要打开**；或运行 `xattr -dr com.apple.quarantine /Applications/OpenDict.app`。
- **Linux：** 使用 `.AppImage`（先 `chmod +x`）或 `.deb`。只有 AppImage 能自动更新。

## 开发

需要 Rust（stable）、Node.js 24+ 以及 [Tauri 前置依赖](https://v2.tauri.app/start/prerequisites/)。

```bash
cargo test --workspace
cargo run -p opendict-mdict-fixture --example sample_library -- sample-library
cd apps/desktop && npm ci && npm test
OPENDICT_LIBRARY_FOLDER=../../sample-library npx tauri dev
```

计划与设计：[`docs/roadmap.md`](docs/roadmap.md)、[`DESIGN.md`](DESIGN.md)。发布流程：[`docs/releasing.md`](docs/releasing.md)。

## 许可证

应用采用 [GPL-3.0-or-later](LICENSE)；MDict 解析库 `crates/mdict` 采用 MIT OR Apache-2.0（[ADR-0001](docs/adr/0001-gpl-app-permissive-parser.md)）。OpenDict 不附带任何词典。
