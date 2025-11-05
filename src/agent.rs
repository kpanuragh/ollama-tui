use crate::models::AgentCommand;
use regex::Regex;
use std::env;
use std::path::PathBuf;

pub struct Agent;

/// System context information for agent mode
#[derive(Debug)]
pub struct SystemContext {
    pub current_dir: String,
    pub is_git_repo: bool,
    pub git_branch: Option<String>,
    pub shell: String,
    pub os: String,
    pub home_dir: Option<String>,
}

impl Agent {
    /// Parses shell commands from AI response text
    /// Looks for bash/sh/shell code blocks in markdown format
    ///
    /// Examples of formats it recognizes:
    /// ```bash
    /// ls -la
    /// ```
    ///
    /// ```sh
    /// pwd
    /// ```
    pub fn parse_commands_from_response(response: &str) -> Vec<AgentCommand> {
        let mut commands = Vec::new();

        // Regex to match code blocks with bash/sh/shell language tags
        let code_block_pattern = Regex::new(r"```(?:bash|sh|shell)\s*\n([\s\S]*?)```").unwrap();

        for cap in code_block_pattern.captures_iter(response) {
            if let Some(code) = cap.get(1) {
                let code_text = code.as_str().trim();

                // Split by newlines to handle multiple commands in one block
                for line in code_text.lines() {
                    let line = line.trim();

                    // Skip empty lines and comments
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }

                    commands.push(AgentCommand::new(line.to_string()));
                }
            }
        }

        commands
    }

    /// Executes a shell command and returns the output or error
    pub async fn execute_command(command: &str) -> Result<String, String> {
        use tokio::process::Command;

        // Execute command in shell
        let output = Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()
            .await
            .map_err(|e| format!("Failed to execute command: {}", e))?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            let mut result = String::new();
            if !stdout.is_empty() {
                result.push_str(&stdout);
            }
            if !stderr.is_empty() {
                if !result.is_empty() {
                    result.push('\n');
                }
                result.push_str(&stderr);
            }

            Ok(if result.is_empty() {
                "[Command executed successfully with no output]".to_string()
            } else {
                result
            })
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(if stderr.is_empty() {
                format!("Command failed with exit code: {}", output.status)
            } else {
                stderr
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_bash_block() {
        let response = r#"
Here's how to list files:

```bash
ls -la
```
"#;
        let commands = Agent::parse_commands_from_response(response);
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].command, "ls -la");
    }

    #[test]
    fn test_parse_multiple_commands() {
        let response = r#"
Let me help you:

```bash
pwd
ls -la
echo "Hello"
```
"#;
        let commands = Agent::parse_commands_from_response(response);
        assert_eq!(commands.len(), 3);
        assert_eq!(commands[0].command, "pwd");
        assert_eq!(commands[1].command, "ls -la");
        assert_eq!(commands[2].command, "echo \"Hello\"");
    }

    #[test]
    fn test_parse_multiple_blocks() {
        let response = r#"
First, check the directory:
```bash
pwd
```

Then list files:
```sh
ls
```
"#;
        let commands = Agent::parse_commands_from_response(response);
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].command, "pwd");
        assert_eq!(commands[1].command, "ls");
    }

    #[test]
    fn test_skip_comments() {
        let response = r#"
```bash
# This is a comment
ls -la
# Another comment
pwd
```
"#;
        let commands = Agent::parse_commands_from_response(response);
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].command, "ls -la");
        assert_eq!(commands[1].command, "pwd");
    }

    #[test]
    fn test_no_commands() {
        let response = "Just some text without any code blocks";
        let commands = Agent::parse_commands_from_response(response);
        assert_eq!(commands.len(), 0);
    }
}

impl Agent {
    /// Gathers system context for agent mode
    pub fn gather_system_context() -> SystemContext {
        let current_dir = env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        let home_dir = env::var("HOME")
            .or_else(|_| env::var("USERPROFILE"))
            .ok();

        let shell = env::var("SHELL")
            .unwrap_or_else(|_| "sh".to_string());

        let os = if cfg!(target_os = "linux") {
            "Linux"
        } else if cfg!(target_os = "macos") {
            "macOS"
        } else if cfg!(target_os = "windows") {
            "Windows"
        } else {
            "Unknown"
        }.to_string();

        // Check if current directory is a git repository
        let is_git_repo = std::path::Path::new(&current_dir)
            .join(".git")
            .exists();

        let git_branch = if is_git_repo {
            std::process::Command::new("git")
                .args(&["rev-parse", "--abbrev-ref", "HEAD"])
                .output()
                .ok()
                .and_then(|output| {
                    if output.status.success() {
                        String::from_utf8(output.stdout).ok()
                            .map(|s| s.trim().to_string())
                    } else {
                        None
                    }
                })
        } else {
            None
        };

        SystemContext {
            current_dir,
            is_git_repo,
            git_branch,
            shell,
            os,
            home_dir,
        }
    }

    /// Creates a system prompt for agent mode with context
    pub fn create_agent_system_prompt(context: &SystemContext) -> String {
        format!(r#"You are an AI assistant in AGENT MODE. You can suggest shell commands to help the user.

SYSTEM CONTEXT:
- Operating System: {}
- Shell: {}
- Current Directory: {}
- Git Repository: {}{}
- Home Directory: {}

IMPORTANT INSTRUCTIONS:
1. When suggesting commands, you MUST use this EXACT format:
   
   ```bash
   command here
   ```

2. Each command must be in a separate bash code block
3. Use 'bash', 'sh', or 'shell' as the language tag
4. Do NOT use other language tags (python, javascript, etc.) for shell commands
5. Each line in a code block becomes a separate command
6. Comments (lines starting with #) are ignored
7. The user will review and approve commands before execution

COMMAND GUIDELINES:
- Suggest safe, read-only commands when possible
- Explain what each command does
- Consider the current directory context
- For git repos, you can suggest git commands
- Use appropriate commands for {}
- Commands run independently (no state between commands)
- Avoid commands that require user interaction
- Use absolute paths or be aware of current directory: {}

EXAMPLE RESPONSE:
"To list all Rust files in the current directory:

```bash
find . -name "*.rs" -type f
```

This will recursively search for all .rs files."

Remember: Commands will be extracted from ```bash blocks and shown to the user for approval before execution."#,
            context.os,
            context.shell,
            context.current_dir,
            if context.is_git_repo { "Yes" } else { "No" },
            if let Some(ref branch) = context.git_branch {
                format!(" (branch: {})", branch)
            } else {
                String::new()
            },
            context.home_dir.as_deref().unwrap_or("unknown"),
            context.os,
            context.current_dir
        )
    }
}
