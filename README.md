# NoneBot CLI (Rust Implementation)

A fast and efficient Rust implementation of the NoneBot command-line interface for managing NoneBot2 projects, plugins, and adapters.

## 🚀 Features

- **Fast Package Management**: Uses [uv](https://astral.sh/blog/uv) for lightning-fast Python package installation
- **Project Management**: Initialize, create, and manage NoneBot2 projects
- **Plugin Management**: Install, uninstall, and update NoneBot2 plugins
- **Adapter Management**: Manage NoneBot2 adapters for different platforms
- **Environment Checking**: Validate Python environment and dependencies
- **Template Generation**: Generate boilerplate code for bots and plugins

## 📋 Prerequisites

### Required
- **Rust** (1.70+) - for building the CLI tool
- **Python** (3.8+) - for running NoneBot2 projects
- **uv** - for Python package management

### Install uv
```bash
# Install uv (recommended method)
curl -LsSf https://astral.sh/uv/install.sh | sh

# Or via pip
pip install uv
```

## 🛠 Installation

```bash
# Clone the repository
git clone https://github.com/your-org/nbr.git
cd nbr

# Build the project
cargo build --release

# Install globally (optional)
cargo install --path .
```

## 📖 Usage

### Project Management

```bash
# Initialize a new NoneBot2 project
nbrinit my-bot

# Create project files
nbrcreate

# Run the bot
nbrrun
```

### Plugin Management

```bash
# Install a plugin
nbrplugin install nonebot-plugin-echo

# Install with specific index
nbrplugin install nonebot-plugin-echo --index https://pypi.org/simple/

# Uninstall a plugin
nbrplugin uninstall nonebot-plugin-echo

# List installed plugins
nbrplugin list

# Update plugins
nbrplugin update
```

### Adapter Management

```bash
# Install an adapter
nbradapter install nonebot2[fastapi]

# Uninstall an adapter
nbradapter uninstall fastapi

# List available adapters
nbradapter list
```

### Environment Management

```bash
# Check environment status
nbrenv

# Generate code templates
nbrgenerate plugin my-plugin
nbrgenerate adapter my-adapter
```

## 🔄 Migration from pip

This project has been migrated from pip to uv for improved performance and reliability. Key changes:

### For Users
- **Install uv**: Required before using the updated CLI
- **Faster operations**: Package installation is 10-100x faster
- **Better resolution**: More reliable dependency management
- **Same commands**: CLI interface remains unchanged

### For Developers
- All `pip install` operations now use `uv add`
- Package information retrieved via `uv pip show`
- Environment checks now verify uv availability
- Templates updated to reference uv commands

See [Migration Documentation](docs/pip-to-uv-migration.md) for detailed information.

## 🏗 Development

### Setup Development Environment

```bash
# Clone and enter directory
git clone https://github.com/your-org/nbr.git
cd nbr

# Install development dependencies
cargo install cargo-watch

# Run tests
cargo test

# Run with hot reload during development
cargo watch -x run
```

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

### Testing Migration

Run the included migration test to verify the pip→uv transition:

```bash
./test_migration.sh
```

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Run tests (`cargo test`)
5. Run migration tests (`./test_migration.sh`)
6. Commit your changes (`git commit -m 'Add amazing feature'`)
7. Push to the branch (`git push origin feature/amazing-feature`)
8. Open a Pull Request

## 📝 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- [NoneBot2](https://github.com/nonebot/nonebot2) - The original NoneBot framework
- [uv](https://github.com/astral-sh/uv) - Fast Python package installer
- Original nbrPython implementation

## 📞 Support

- **Issues**: [GitHub Issues](https://github.com/your-org/nbr/issues)
- **Documentation**: [docs/](docs/)
- **NoneBot Community**: [NoneBot Documentation](https://v2.nonebot.dev/)

---

**Note**: This is a Rust reimplementation of the NoneBot CLI with enhanced performance through uv integration. For the original Python version, see the [official NoneBot CLI](https://github.com/nonebot/nbr).