<div align="center">

[![nbr](https://socialify.git.ci/fllesser/nbr/image?description=1&font=Bitter&language=1&logo=https%3A%2F%2Fnonebot.dev%2Flogo.png&name=1&owner=1&pattern=Circuit+Board&theme=Light)](https://github.com/fllesser/nbr)

</div>

rust 实现的 NoneBot 命令行工具，用于管理 NoneBot2 项目、插件和适配器。

### 注意：该项目仍在开发中，部分功能可能尚未完全实现。

## 🚀 特性

- **快速包管理**：使用 [uv](https://astral.sh/blog/uv) 进行闪电般的 Python 包安装
- **项目管理**：初始化、创建和管理 NoneBot2 项目
- **插件管理**：安装、卸载和更新 NoneBot2 插件
- **适配器管理**：管理 NoneBot2 适配器
- **环境检查**：验证 Python 环境
- **模板生成**：生成 NoneBot2 项目和插件的样板代码
- **兼容性**：兼容 [nb-cli](https://github.com/nonebot/nb-cli), 不会破坏原有结构

## 📋 先决条件

### 必需
- **uv** (0.8.3+) - 用于 Python 依赖管理

### 安装 uv
<details>
<summary>使用 astral 官方安装脚本(推荐)</summary>

    curl -LsSf https://astral.sh/uv/install.sh | sh

</details>

## 🛠 安装

<details>
<summary>使用 cargo 安装</summary>

    cargo install nbr

</details>

<details>
<summary>从 releases 安装</summary>

仅支持 Linux(x86_64), MacOS(arm64) 和 Windows(x86_64)

<details>
<summary>Linux(x86_64) 安装</summary>

从 GitHub Releases 下载最新版本

    curl -LsSf https://github.com/fllesser/nbr/releases/latest/download/nbr-Linux-musl-x86_64.tar.gz | tar -xzf -

将二进制文件移动到 PATH

    sudo mv nbr /usr/local/bin/
</details>

<details>
<summary>MacOS(arm64)</summary>

从 GitHub Releases 下载最新版本

    curl -LsSf https://github.com/fllesser/nbr/releases/latest/download/nbr-macOS-arm64.tar.gz | tar -xzf -

将二进制文件移动到 PATH

    sudo mv nbr /Users/{username}/.local/bin/
</details>

<details>
<summary>Windows(x86_64)</summary>

从 GitHub Releases 下载最新版本

    curl -LsSf https://github.com/fllesser/nbr/releases/latest/download/nbr-Windows-msvc-x86_64.zip | tar -xzf -

将二进制文件移动到 PATH

    ...
</details>

</details>

<details>
<summary>安装仓库最新分支</summary>

    cargo install --git https://github.com/fllesser/nbr.git

</details>


## 📖 使用


<details>
<summary>项目管理</summary>

创建一个新的 NoneBot2 项目，选项 `-p` / `--python` 指定 Python 版本

    nbr create

运行 NoneBot2 项目，选项 `-r` / `--reload` 重新加载项目

    nbr run

</details>

<details>
<summary>插件管理</summary>

安装一个插件

    nbr plugin install nonebot-plugin-emojilike

安装一个插件，指定索引

    nbr plugin install nonebot-plugin-emojilike --index https://pypi.org/simple/

从 github 仓库安装一个插件

    nbr plugin install https://github.com/fllesser/nonebot-plugin-abs@master

卸载一个插件

    nbr plugin uninstall nonebot-plugin-emojilike

更新一个插件，选项 `-r` / `--reinstall` 重新安装这个插件

    nbr plugin update <plugin>

更新所有插件

    nbr plugin update --all

列出所有已安装的插件，选项 `--outdated` 列出过时的插件

    nbr plugin list

</details>

<details>
<summary>适配器管理</summary>

安装适配器

    nbr adapter install

卸载适配器

    nbr adapter uninstall

列出所有已安装的适配器，选项 `-a` / `--all` 列出所有已安装的适配器

    nbr adapter list

</details>


<details>
<summary>环境管理</summary>

检查环境状态

    nbr env check

打印环境信息

    nbr env info

</details>


## 🤝 贡献

1. Fork 仓库
2. 创建一个功能分支 (`git checkout -b feature/amazing-feature`)
3. 进行修改
4. 运行测试 (`cargo test`)
5. 提交修改 (`git commit -m 'Add amazing feature'`)
6. 推送到分支 (`git push origin feature/amazing-feature`)
7. 打开一个 Pull Request

## 📝 许可证

这个项目使用 MIT 许可证 - 详情请参阅 [LICENSE](LICENSE) 文件

## 🙏 致谢

- [NoneBot2](https://github.com/nonebot/nonebot2) - NoneBot2 框架
- [uv](https://github.com/astral-sh/uv) - Python 依赖管理工具

## 📞 支持

- **Issues**: [GitHub Issues](https://github.com/fllesser/nbr/issues)
- **NoneBot Community**: [NoneBot Documentation](https://v2.nonebot.dev/)

---

**注意**: 这是一个 Rust 实现的 NoneBot CLI。对于原始的 Python 版本，请参阅 [官方 NoneBot CLI](https://github.com/nonebot/nb-cli)。