# Ollama-Chat

Ollama-Chat is a Terminal User Interface (TUI) chat system built with Rust, powered by Ollama for AI chat models.

## Features

- **Basic Authentication** for Ollama server.
- **External Ollama Server Support** (connect to remote instances).
- **TUI-based Model Selection** (choose models inside the interface).
- **Automatic Configuration File** (`config.json`) stored in default OS directories.
- **No environment variable support** – all settings are in `config.json`.
- **No CLI parameters** – everything is handled via the TUI.

## Installation (Build from Source)

### 1. Install Rust

Ensure Rust and Cargo are installed:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

Verify installation:

```sh
rustc --version
cargo --version
```

### 2. Clone & Build the Project

```sh
git clone https://github.com/kpanuragh/ollama-chat.git
cd ollama-chat
cargo build --release
```

Move the binary to a system-wide location:

```sh
mv target/release/ollama-chat /usr/local/bin/
```

Run the chat system:

```sh
ollama-chat
```

## Configuration (`config.json`)

When you run `ollama-chat` for the first time, it automatically creates a `config.json` file in the default OS configuration directory.

### Configuration File Location

- **Linux:** `$HOME/.config/ollama-chat/config.json`
- **macOS:** `$HOME/Library/Application Support/ollama-chat/config.json`
- **Windows:** `%APPDATA%\ollama-chat\config.json`

Once created, you can manually edit `config.json` to change settings.

### Configuration Options

```json
{
  "domain": "http://localhost:11434",
  "username": "your_username",
  "password": "your_password"
}
```

- **domain**: URL of the Ollama server (local or remote).
- **username**: Your authentication username.
- **password**: Your authentication password.

> **Note:** No environment variable support – all values must be set in `config.json`.

## External Ollama Server Support

To use a remote Ollama server, update the `domain` in `config.json`:

```json
{
  "domain": "http://your-ollama-server:port",
  "username": "your_username",
  "password": "your_password"
}
```

## Model Selection (via TUI)

- No CLI parameters for model selection.
- You can choose the model directly inside the TUI interface.
- Available models depend on what’s installed in your Ollama server.

## Usage

Run the chat system:

```sh
ollama-chat
```

> No `--help` or additional CLI commands – everything is handled inside the TUI.

## Controls

- **Press `c`** to move to chat mode.
- **Press `m`** for model selection mode.
- **Press `Escape`** to go back to control mode.

## License

This project is licensed under the **MIT License**.

## Contributing & Support

- **Found a bug?** Open an issue on GitHub: [Issues](https://github.com/kpanuragh/ollama-chat/issues)
- **Want to contribute?** Submit a Pull Request!
- **Need help?** Contact us in the [Discussions](https://github.com/kpanuragh/ollama-chat/discussions) section.
