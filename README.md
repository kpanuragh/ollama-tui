
# Ollama TUI 🚀

A feature-rich, terminal-based user interface for interacting with [Ollama](https://ollama.com/ "null"), written in Rust. Enjoy a polished and responsive chat experience directly from your command line.

![Screenshot](https://raw.githubusercontent.com/kpanuragh/ollama-chat/main/ollama.png)

## ✨ Features

-   **Polished** Terminal **UI**: A clean, responsive interface built with `ratatui`.
    
-   **Streaming Responses**: Get instant feedback as the model generates its response token by token.
    
-   **Persistent Chat History**: All conversations are automatically saved to a local SQLite database and reloaded on startup.
    
-   **Chat Session Management**:
    
    -   Organize conversations into separate, persistent sessions.
        
    -   Create new sessions on the fly (`Ctrl+N`).
        
    -   Easily switch between sessions (`Tab`).
        
    -   Clear the history of the current session (`Ctrl+D`).
        
-   **Dynamic Model Switching**: Pop-up interface (`Ctrl+L`) to switch between any of your available Ollama models.
    
-   **Secure & Configurable**:
    
    -   **External Configuration**: Settings are stored in a `config.json` file in the standard OS configuration directory.
        
    -   **Flexible Authentication**: Supports both `Bearer Token` and `Basic` (username/password) authentication for connecting to a secured Ollama server.
        
-   **Cross-Platform**: Runs on Linux, macOS, and Windows.
    

## 🔧 Installation & Setup

### Prerequisites

1.  **Rust**: Ensure you have the Rust toolchain installed. You can get it from [rust-lang.org](https://www.rust-lang.org/tools/install "null").
    
2.  **Ollama**: The Ollama server must be installed and running on your machine or a remote server. Get it from [ollama.com](https://ollama.com/ "null").
    
3.  **An Ollama Model**: Pull at least one model. For example:
    
    ```
    ollama run llama3
    
    ```
    

### Building from Source

1.  **Clone the repository:**
    
    ```
    git clone <your-repo-url>
    cd ollama-tui
    
    ```
    
2.  **Build the application:**
    
    ```
    cargo build --release
    
    ```
    
3.  **Run the application:**
    
    ```
    ./target/release/ollama-tui
    
    ```
    

## ⚙️ Configuration

On the first run, the application will create a `config.json` file in your system's default configuration directory:

-   **Linux**: `~/.config/ollama-tui/config.json`
    
-   **macOS**: `~/Library/Application Support/com.rust-tui.ollama-tui/config.json`
    
-   **Windows**: `C:\Users\<YourUser>\AppData\Roaming\rust-tui\ollama-tui\config\config.json`
    

You can edit this file to configure the Ollama server connection, database path, and authentication.

### Default Configuration

```
{
  "ollama_host": "[http://127.0.0.1](http://127.0.0.1)",
  "ollama_port": 11434,
  "db_filename": "ollama-tui.sqlite",
  "auth_enabled": false,
  "auth_method": null
}

```

### Authentication Examples

#### Bearer Token

```
{
  "ollama_host": "[https://your.remote.ollama.host](https://your.remote.ollama.host)",
  "ollama_port": 443,
  "db_filename": "ollama-tui.sqlite",
  "auth_enabled": true,
  "type": "bearer",
  "token": "your-secret-api-token"
}

```

#### Basic Authentication

```
{
  "ollama_host": "[https://your.remote.ollama.host](https://your.remote.ollama.host)",
  "ollama_port": 443,
  "db_filename": "ollama-tui.sqlite",
  "auth_enabled": true,
  "type": "basic",
  "username": "your-username",
  "password": "your-secure-password"
}

```

## ⌨️ Keybindings

Key

Action

Context

`Ctrl` + `C`

Quit the application

Global

`Ctrl` + `N`

Create a new chat session

Global

`Ctrl` + `D`

**Delete** all messages in current session

Normal Mode

`Ctrl` + `L`

Open the **M**odel selection popup

Global

`Tab`

Switch focus to the Sessions panel

Normal Mode

`Esc` or `q`

Return to Normal Mode from a popup/panel

Model/Session panels

`Enter`

Send message / Confirm selection

Normal Mode / Popups

`Up`/`Down` Arrow

Navigate lists / Scroll chat history

All

Any other key

Type in the input box

Normal Mode

## 🤝 Contributing

Contributions are welcome! If you have ideas for new features, bug fixes, or improvements, feel free to open an issue or submit a pull request.

## 📄 License

This project is licensed under the MIT License. See the `LICENSE` file for details.

