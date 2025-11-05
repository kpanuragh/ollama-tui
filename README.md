
# Ollama TUI 🚀

A feature-rich, terminal-based user interface for interacting with [Ollama](https://ollama.com/), written in Rust. Enjoy a polished and responsive chat experience with **vim-style navigation** directly from your command line.

![Screenshot](https://raw.githubusercontent.com/kpanuragh/ollama-chat/main/ollama.png)

## ✨ Features

- **🎯 Vim-Style Interface**: Full vim-like modal editing with Normal, Insert, Command, and Visual modes
- **✂️ Visual Mode**: Select and copy chat text with vim-style visual selection
- **💬 Multiple Chat Sessions**: Create, switch between, and manage multiple persistent chat sessions
- **⚡ Streaming Responses**: Get instant feedback as the model generates responses token by token
- **💾 Persistent History**: All conversations automatically saved to local SQLite database
- **🔄 Dynamic Model Switching**: Seamless switching between available Ollama models
- **🎨 Themeable Interface**: Customizable colors and themes
- **🔐 Secure Authentication**: Support for Bearer Token and Basic authentication
- **📱 Cross-Platform**: Runs on Linux, macOS, and Windows
- **⌨️ Keyboard-Driven**: Complete navigation without touching the mouse
- **🤖 Agent Mode**: AI-powered command execution with approval workflow

## 🎮 Vim-Style Interface

### Modes

- **Normal Mode** (`-- NORMAL --`): Navigate and issue commands
- **Insert Mode** (`-- INSERT --`): Type messages and chat with AI
- **Command Mode** (`-- COMMAND --`): Execute vim-style commands
- **Visual Mode** (`-- VISUAL --`): Select and copy chat text
- **Agent Mode** (`-- AGENT --`): Request AI to suggest shell commands
- **Agent Approval Mode** (`-- AGENT APPROVAL --`): Review and approve/reject commands
- **Help Mode** (`-- HELP --`): Comprehensive help system

### Key Bindings

#### Normal Mode
| Key | Action |
|-----|--------|
| `i` | Enter insert mode |
| `o`/`O` | Enter insert mode (clear input) |
| `v` | Enter visual mode (select text) |
| `:` | Enter command mode |
| `?` | Show help popup |
| `q` | Quick quit |
| `j`/`↓` | Scroll down in chat |
| `k`/`↑` | Scroll up in chat |
| `g` | Go to top of chat |
| `G` | Go to bottom of chat |
| `PgUp`/`PgDn` | Page up/down |

#### Insert Mode
| Key | Action |
|-----|--------|
| `ESC` | Return to normal mode |
| `Enter` | Send message |
| `Backspace` | Delete character |
| *Any character* | Type message |

#### Visual Mode
| Key | Action |
|-----|--------|
| `j`/`↓` | Extend selection down |
| `k`/`↑` | Extend selection up |
| `g` | Go to top (start of chat) |
| `G` | Go to bottom (end of chat) |
| `PgUp`/`PgDn` | Page up/down |
| `y` | Copy selection to clipboard |
| `ESC`/`q` | Return to normal mode |

#### Command Mode
| Command | Action |
|---------|--------|
| `:q` | Quit application |
| `:w` | Save current session |
| `:wq` | Save and quit |
| `:n` | Create new session |
| `:c` | Clear current session |
| `:m` | Select model |
| `:s` | Select session |
| `:a` | Enter agent mode |
| `:h` or `:?` | Show help |
| `:d` | Delete current session |
| `:d<N>` | Delete session N |
| `:b<N>` | Switch to session N |

## 🔧 Installation & Setup

### Prerequisites

1. **Rust**: Install from [rust-lang.org](https://www.rust-lang.org/tools/install)
2. **Ollama**: Install and run from [ollama.com](https://ollama.com/)
3. **An Ollama Model**: Pull at least one model:
   ```bash
   ollama run llama3
   ```

#### Clipboard Support (Optional)
For Visual mode copy functionality, install one of these clipboard utilities:
- **Linux**: `xclip`, `xsel`, or `wl-copy` (Wayland)
- **macOS**: `pbcopy` (built-in)
- **Windows**: `clip` (built-in)

### Building from Source

1. **Clone the repository:**
   ```bash
   git clone <your-repo-url>
   cd ollama-tui
   ```

2. **Build the application:**
   ```bash
   cargo build --release
   ```

3. **Run the application:**
   ```bash
   ./target/release/ollama-tui
   ```

### Using Nix (Recommended)

If you have Nix with flakes enabled:

```bash
nix develop
cargo run
```

## ⚙️ Configuration

Configuration file is automatically created at:

- **Linux**: `~/.config/ollama-tui/config.json`
- **macOS**: `~/Library/Application Support/com.rust-tui.ollama-tui/config.json`
- **Windows**: `%APPDATA%\rust-tui\ollama-tui\config\config.json`

### Default Configuration

```json
{
  "ollama_host": "http://127.0.0.1",
  "ollama_port": 11434,
  "db_filename": "ollama-tui.sqlite",
  "auth_enabled": false,
  "auth_method": null,
  "theme": {
    "chat_border_color": "blue",
    "sessions_border_color": "green",
    "user_message_color": "cyan",
    "assistant_message_color": "white",
    "status_bar_color": "gray",
    "highlight_color": "black",
    "highlight_bg_color": "white",
    "popup_border_color": "yellow"
  }
}
```

### Authentication Examples

#### Bearer Token
```json
{
  "ollama_host": "https://your.remote.ollama.host",
  "ollama_port": 443,
  "auth_enabled": true,
  "auth_method": {
    "type": "bearer",
    "token": "your-secret-api-token"
  }
}
```

#### Basic Authentication
```json
{
  "ollama_host": "https://your.remote.ollama.host",
  "ollama_port": 443,
  "auth_enabled": true,
  "auth_method": {
    "type": "basic",
    "username": "your-username",
    "password": "your-secure-password"
  }
}
```

## 🤖 Agent Mode

Agent mode allows the AI to suggest shell commands based on your requests, which you can then review and approve before execution.

### How Agent Mode Works

1. **Enter Agent Mode**: Type `:a` from normal mode
2. **Make a Request**: Ask the AI to perform a task (e.g., "list all files in the src directory")
3. **AI Suggests Commands**: The AI responds with suggested shell commands in code blocks
4. **Review & Approve**: Commands are parsed and shown in the approval interface
5. **Execute**: Approved commands are executed and results are shown in chat

### Agent Approval Mode Keys

| Key | Action |
|-----|--------|
| `j`/`k` or `↑`/`↓` | Navigate through commands |
| `y` | Approve current command |
| `n` | Reject current command |
| `a` | Approve all commands |
| `r` | Reject all commands |
| `x` or `Enter` | Execute approved commands |
| `ESC` or `q` | Cancel and return to agent mode |

### Example Agent Mode Usage

```
User: "Show me the size of all rust files"
AI: "Here are the commands to show rust file sizes:

```bash
find . -name "*.rs" -exec du -h {} \;
```
"

[Approval popup appears showing the command]
[Press 'y' to approve, then 'x' to execute]
[Output appears in chat]
```

### Safety Features

- **Explicit Approval**: No commands execute without your approval
- **Visual Indicators**: Approved commands show ✓, rejected show ✗
- **Command Review**: See exactly what will be executed before running
- **Cancellation**: Press ESC at any time to cancel

## 🚀 Quick Start Guide

1. **Start the application** - You'll be in Normal mode
2. **Press `i`** to enter Insert mode and type your first message
3. **Press `Enter`** to send the message
4. **Press `ESC`** to return to Normal mode
5. **Press `v`** to enter Visual mode and select text to copy
6. **Press `:`** to enter Command mode and try:
   - `:n` to create a new session
   - `:m` to select a different model
   - `:s` to switch between sessions
   - `:a` to try agent mode
   - `:?` for comprehensive help

## 💡 Tips & Tricks

- **Multiple Sessions**: Use `:n` to create topic-specific chat sessions
- **Visual Mode**: Press `v` to select and copy chat text with vim-style selection
- **Quick Copy**: In Visual mode, select text with `j/k` and press `y` to copy to clipboard
- **Quick Navigation**: Use `g` and `G` to jump to top/bottom of long chats
- **Session Management**: Use `:b1`, `:b2`, etc. to quickly switch to specific sessions
- **Model Switching**: Use `:m` to change AI models mid-conversation
- **Agent Mode**: Use `:a` to let AI suggest and execute shell commands with your approval
- **Help System**: Press `?` from Normal mode for complete command reference

## 🎨 Customization

### Themes
Edit the `theme` section in your config.json to customize colors:

```json
"theme": {
  "chat_border_color": "magenta",
  "sessions_border_color": "cyan",
  "user_message_color": "green",
  "assistant_message_color": "yellow"
}
```

Available colors: `black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `white`, `gray`

## 🤝 Contributing

Contributions are welcome! Areas for improvement:

- Additional vim-style commands and visual mode enhancements
- Theme system enhancements
- Agent mode development
- Performance optimizations
- Additional authentication methods
- Enhanced clipboard support and text manipulation features

## 📄 License

This project is licensed under the MIT License. See the `LICENSE` file for details.

---

**Made with ❤️ for terminal enthusiasts and vim lovers**

