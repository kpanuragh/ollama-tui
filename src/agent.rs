use crate::models::AgentCommand;
use regex::Regex;

pub struct Agent;

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
