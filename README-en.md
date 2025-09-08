# NoneBot CLI (Rust Implementation)

A fast and efficient Rust implementation of the NoneBot command-line interface for managing NoneBot2 projects, plugins, and adapters.

### Note: This project is still under development, and some features may not be fully functional.

## 🚀 Features

- **Fast Package Management**: Uses [uv](https://astral.sh/blog/uv) for lightning-fast Python package installation
- **Project Management**: Initialize, create, and manage NoneBot2 projects
- **Plugin Management**: Install, uninstall, and update NoneBot2 plugins
- **Adapter Management**: Manage NoneBot2 adapters for different platforms
- **Environment Checking**: Validate Python environment and dependencies
- **Template Generation**: Generate boilerplate code for bots and plugins

## 📋 Prerequisites

### Required
- **Rust** (1.85+) - for building the CLI tool
- **Python** (3.10+) - for running NoneBot2 projects
- **uv** (0.80+) - for Python package management

### Install uv
<details>
<summary>Use astral official install script(recommended)</summary>

    curl -LsSf https://astral.sh/uv/install.sh | sh

</details>

## 🛠 Installation

<details>
<summary>Install with cargo</summary>

    cargo install nbr

</details>

<details>
<summary>Install from releases</summary>

Only support Linux(x86_64), MacOS(arm64) and Windows(x86_64)

<details>
<summary>Linux(x86_64)</summary>

Download the latest release from GitHub

    curl -LsSf https://github.com/fllesser/nbr/releases/latest/download/nbr-Linux-musl-x86_64.tar.gz | tar -xzf -

Move the binary to your PATH

    sudo mv nbr /usr/local/bin/
</details>

<details>
<summary>MacOS(arm64)</summary>

Download the latest release from GitHub

    curl -LsSf https://github.com/fllesser/nbr/releases/latest/download/nbr-macOS-arm64.tar.gz | tar -xzf -

Move the binary to your PATH

    sudo mv nbr /Users/{username}/.local/bin/
</details>

<details>
<summary>Windows(x86_64)</summary>

Download the latest release from GitHub

    curl -LsSf https://github.com/fllesser/nbr/releases/latest/download/nbr-Windows-msvc-x86_64.zip | tar -xzf -

Move the binary to your PATH

    ...
</details>

</details>

<details>
<summary>Install with repository</summary>
Clone the repository

    git clone https://github.com/fllesser/nbr.git

Install globally

    cargo install --path .

</details>


## 📖 Usage


<details>
<summary>Project Management</summary>

Create a new NoneBot2 project, Option `-p` / `--python` to specify the Python version

    nbr create

Run NoneBot2 project, Option `-r` / `--reload` to reload the project

    nbr run

</details>

<details>
<summary>Plugin Management</summary>

Install a plugin

    nbr plugin install nonebot-plugin-emojilike

Install a plugin with specific index

    nbr plugin install nonebot-plugin-emojilike --index https://pypi.org/simple/

Install a plugin from github repo

    nbr plugin install https://github.com/fllesser/nonebot-plugin-abs@master

Uninstall a plugin

    nbr plugin uninstall nonebot-plugin-emojilike

Update plugins, Option `-r` / `--reinstall` to reinstall this plugin

    nbr plugin update <plugin>

Update all plugins

    nbr plugin update --all

List installed plugins, Option `--outdated` to list outdated plugins

    nbr plugin list

</details>

<details>
<summary>Adapter Management</summary>

Install adapters

    nbr adapter install

Uninstall adapters

    nbr adapter uninstall

List installed adapters, Option `-a` / `--all` to list all installed adapters

    nbr adapter list

</details>


<details>
<summary>Environment Management</summary>

Check environment status

    nbr env check

Print environment information

    nbr env info

</details>


## 🏗 Development

### Project Structure

```
src/
├── cli/
│   ├── adapter.rs      # Adapter management
│   ├── create.rs       # Project creation
│   ├── env.rs          # Environment checking
│   ├── generate.rs     # Code generation
│   ├── init.rs         # Project initialization
│   ├── plugin.rs       # Plugin management
│   └── run.rs          # Bot execution
├── config.rs           # Configuration management
├── error.rs            # Error handling
├── main.rs             # CLI entry point
└── utils.rs            # Utility functions
```

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Run tests (`cargo test`)
5. Commit your changes (`git commit -m 'Add amazing feature'`)
6. Push to the branch (`git push origin feature/amazing-feature`)
7. Open a Pull Request

## 📝 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- [NoneBot2](https://github.com/nonebot/nonebot2) - The original NoneBot framework
- [uv](https://github.com/astral-sh/uv) - Fast Python package installer

## 📞 Support

- **Issues**: [GitHub Issues](https://github.com/fllesser/nbr/issues)
- **Documentation**: [docs/](docs/)
- **NoneBot Community**: [NoneBot Documentation](https://v2.nonebot.dev/)

---

**Note**: This is a Rust reimplementation of the NoneBot CLI with enhanced performance through uv integration. For the original Python version, see the [official NoneBot CLI](https://github.com/nonebot/nb-cli).