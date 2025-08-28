use crate::models::{AgentCommand, RiskLevel};
use anyhow::{anyhow, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command as AsyncCommand;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandHistory {
    pub commands: Vec<HistoryEntry>,
    pub favorites: Vec<String>,
    pub usage_stats: HashMap<String, u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub command: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub success: bool,
    pub execution_time_ms: u64,
    pub working_dir: String,
}

pub struct Agent;

#[derive(Debug, Clone)]
pub struct CommandResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub execution_time_ms: u64,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct SystemContext {
    pub current_dir: String,
    pub is_git_repo: bool,
    pub project_type: ProjectType,
    pub available_tools: Vec<String>,
    pub os_type: String,
    pub user_permissions: PermissionLevel,
}

#[derive(Debug, Clone)]
pub enum ProjectType {
    Rust,
    NodeJs,
    Python,
    Go,
    Generic,
}

#[derive(Debug, Clone)]
pub enum PermissionLevel {
    User,
    Sudo,
    Root,
}

impl CommandHistory {
    /// Creates a new, empty `CommandHistory` instance with no commands, favorites, or usage statistics.
    ///
    /// # Examples
    ///
    /// ```
    /// let history = CommandHistory::new();
    /// assert!(history.commands.is_empty());
    /// assert!(history.favorites.is_empty());
    /// assert!(history.usage_stats.is_empty());
    /// ```
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            favorites: Vec::new(),
            usage_stats: HashMap::new(),
        }
    }
    
    /// Adds a new command entry to the history, recording its outcome, execution time, and working directory.
    ///
    /// Maintains usage statistics for each command and ensures the history contains only the most recent 1000 entries.
    pub fn add_entry(&mut self, command: String, success: bool, execution_time_ms: u64) {
        let entry = HistoryEntry {
            command: command.clone(),
            timestamp: chrono::Utc::now(),
            success,
            execution_time_ms,
            working_dir: env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
        };
        
        self.commands.push(entry);
        *self.usage_stats.entry(command).or_insert(0) += 1;
        
        // Keep only last 1000 commands
        if self.commands.len() > 1000 {
            self.commands.remove(0);
        }
    }
    
    /// Adds a command to the favorites list if it is not already present.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut history = CommandHistory::new();
    /// history.add_favorite("ls -la".to_string());
    /// assert!(history.favorites.contains(&"ls -la".to_string()));
    /// ```
    pub fn add_favorite(&mut self, command: String) {
        if !self.favorites.contains(&command) {
            self.favorites.push(command);
        }
    }
    
    /// Returns the most frequently used commands up to the specified limit, sorted by usage count in descending order.
    ///
    /// # Parameters
    /// - `limit`: The maximum number of popular commands to return.
    ///
    /// # Returns
    /// A vector of tuples containing the command string and its usage count, ordered from most to least used.
    ///
    /// # Examples
    ///
    /// ```
    /// let history = CommandHistory::new();
    /// // ... add entries ...
    /// let popular = history.get_popular_commands(5);
    /// assert!(popular.len() <= 5);
    /// ```
    pub fn get_popular_commands(&self, limit: usize) -> Vec<(String, u32)> {
        let mut commands: Vec<_> = self.usage_stats.iter().collect();
        commands.sort_by(|a, b| b.1.cmp(a.1));
        commands.into_iter()
            .take(limit)
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }
}

impl Agent {
    /// Extracts shell commands from a language model response.
    ///
    /// Searches for commands within fenced code blocks labeled as bash/sh/shell and for inline commands prefixed with `$`.
    /// For each detected command, generates a risk assessment and a human-readable description.
    ///
    /// # Returns
    /// A vector of `AgentCommand` instances representing the parsed commands, each with associated metadata.
    ///
    /// # Examples
    ///
    /// ```
    /// let response = "Try running:\n```bash\nls -la\n```\nOr use `$ git status`.";
    /// let commands = Agent::parse_commands_from_response(response);
    /// assert_eq!(commands.len(), 2);
    /// assert_eq!(commands[0].command, "ls -la");
    /// assert_eq!(commands[1].command, "git status");
    /// ```
    pub fn parse_commands_from_response(response: &str) -> Vec<AgentCommand> {
        let mut commands = Vec::new();
        
        // Look for command blocks in the response
        let command_regex = Regex::new(r"```(?:bash|sh|shell)?\s*\n(.*?)\n```").unwrap();
        
        for captures in command_regex.captures_iter(response) {
            if let Some(command_text) = captures.get(1) {
                let command = command_text.as_str().trim().to_string();
                if !command.is_empty() {
                    let risk_level = Self::assess_risk_level(&command);
                    let description = Self::generate_description(&command);
                    
                    commands.push(AgentCommand {
                        command: command.clone(),
                        description,
                        risk_level,
                        approved: false,
                        executed: false,
                        output: None,
                        error: None,
                    });
                }
            }
        }
        
        // Also look for inline commands with specific markers
        let inline_regex = Regex::new(r"\$\s*([^\n]+)").unwrap();
        for captures in inline_regex.captures_iter(response) {
            if let Some(command_text) = captures.get(1) {
                let command = command_text.as_str().trim().to_string();
                if !command.is_empty() && !commands.iter().any(|c| c.command == command) {
                    let risk_level = Self::assess_risk_level(&command);
                    let description = Self::generate_description(&command);
                    
                    commands.push(AgentCommand {
                        command: command.clone(),
                        description,
                        risk_level,
                        approved: false,
                        executed: false,
                        output: None,
                        error: None,
                    });
                }
            }
        }
        
        commands
    }
    
    /// Determines the risk level of a shell command based on its content.
    ///
    /// Returns `RiskLevel::Critical` for dangerous commands, `High` for system modifications,
    /// `Moderate` for file modifications, and `Safe` for read-only operations.
    ///
    /// # Examples
    ///
    /// ```
    /// let risk = Agent::assess_risk_level("rm -rf /tmp/test");
    /// assert_eq!(risk, RiskLevel::Critical);
    ///
    /// let risk = Agent::assess_risk_level("ls -la");
    /// assert_eq!(risk, RiskLevel::Safe);
    /// ```
    pub fn assess_risk_level(command: &str) -> RiskLevel {
        let command_lower = command.to_lowercase();
        let parts: Vec<&str> = command.split_whitespace().collect();
        
        // Critical commands - immediate danger
        if Self::is_critical_command(&command_lower) {
            return RiskLevel::Critical;
        }
        
        // High risk commands - system modifications
        if Self::is_high_risk_command(&command_lower, &parts) {
            return RiskLevel::High;
        }
        
        // Moderate risk commands - file modifications
        if Self::is_moderate_risk_command(&command_lower, &parts) {
            return RiskLevel::Moderate;
        }
        
        // Safe commands - read-only operations
        RiskLevel::Safe
    }
    
    /// Determines if a shell command matches known critical or destructive patterns.
    ///
    /// Returns `true` if the command contains substrings associated with highly dangerous operations such as disk formatting, partitioning, or recursive file deletion; otherwise, returns `false`.
    ///
    /// # Examples
    ///
    /// ```
    /// assert!(is_critical_command("rm -rf /"));
    /// assert!(!is_critical_command("ls -la"));
    /// ```
    fn is_critical_command(command: &str) -> bool {
        let critical_patterns = [
            "rm -rf",
            "sudo rm",
            "mkfs",
            "dd if=",
            "fdisk",
            "format",
            "del /f",
            "> /dev/",
            "shred",
            "wipefs",
            "parted",
            "cfdisk",
        ];
        
        critical_patterns.iter().any(|&pattern| command.contains(pattern))
    }
    
    /// Determines if a command is considered high risk based on its content.
    ///
    /// High-risk commands include those that start with `sudo`, perform network operations piped to a shell (such as `curl | sh` or `wget | sh`), or involve system service and user management operations (e.g., `systemctl`, `service`, `iptables`, `ufw`, `chmod +x`, `chown`, `passwd`, `useradd`, `userdel`, `groupadd`).
    ///
    /// # Examples
    ///
    /// ```
    /// let risky = is_high_risk_command("sudo apt update", &["sudo", "apt", "update"]);
    /// assert!(risky);
    ///
    /// let risky = is_high_risk_command("curl https://example.com | sh", &["curl", "https://example.com", "|", "sh"]);
    /// assert!(risky);
    ///
    /// let not_risky = is_high_risk_command("ls -la", &["ls", "-la"]);
    /// assert!(!not_risky);
    /// ```
    fn is_high_risk_command(command: &str, parts: &[&str]) -> bool {
        if parts.is_empty() {
            return false;
        }
        
        // Sudo operations
        if command.starts_with("sudo") {
            return true;
        }
        
        // Network operations with execution
        if (command.contains("curl") || command.contains("wget")) && command.contains("| sh") {
            return true;
        }
        
        // System service operations
        let high_risk_commands = [
            "systemctl", "service", "iptables", "ufw", "chmod +x",
            "chown", "passwd", "useradd", "userdel", "groupadd",
        ];
        
        high_risk_commands.iter().any(|&cmd| command.contains(cmd))
    }
    
    /// Determines if a command is considered moderate risk based on its type and usage.
    ///
    /// Returns `true` if the command matches common file operations or package installation commands,
    /// or if it involves output redirection with such commands. Otherwise, returns `false`.
    ///
    /// # Examples
    ///
    /// ```
    /// let is_moderate = is_moderate_risk_command("cp file1 file2", &["cp", "file1", "file2"]);
    /// assert!(is_moderate);
    ///
    /// let not_moderate = is_moderate_risk_command("ls -la", &["ls", "-la"]);
    /// assert!(!not_moderate);
    /// ```
    fn is_moderate_risk_command(command: &str, parts: &[&str]) -> bool {
        if parts.is_empty() {
            return false;
        }
        
        let moderate_commands = [
            "cp", "mv", "mkdir", "touch", "echo", "git push",
            "npm install", "pip install", "cargo install", "rm",
        ];
        
        // Check if command starts with moderate risk command
        moderate_commands.iter().any(|&cmd| {
            command.starts_with(cmd) || 
            (command.contains(cmd) && command.contains(">"))
        })
    }
    
    /// Generates a human-readable description of a shell command.
    ///
    /// Returns a brief summary of the command's purpose, including specific descriptions for common commands and subcommands. If the command is unrecognized, returns a generic description.
    ///
    /// # Examples
    ///
    /// ```
    /// let desc = generate_description("git status");
    /// assert_eq!(desc, "Show git repository status");
    ///
    /// let desc2 = generate_description("ls -la");
    /// assert_eq!(desc2, "List directory contents");
    ///
    /// let desc3 = generate_description("unknowncmd arg");
    /// assert_eq!(desc3, "Execute command: unknowncmd");
    /// ```
    fn generate_description(command: &str) -> String {
        let cmd_parts: Vec<&str> = command.split_whitespace().collect();
        if cmd_parts.is_empty() {
            return "Unknown command".to_string();
        }
        
        match cmd_parts[0] {
            "ls" | "ll" | "la" => "List directory contents".to_string(),
            "cd" => format!("Change directory to {}", cmd_parts.get(1).unwrap_or(&"home")),
            "pwd" => "Show current directory".to_string(),
            "cat" => format!("Display contents of {}", cmd_parts.get(1).unwrap_or(&"file")),
            "grep" => "Search for text patterns".to_string(),
            "find" => "Search for files and directories".to_string(),
            "cp" => "Copy files or directories".to_string(),
            "mv" => "Move or rename files".to_string(),
            "rm" => "Delete files or directories".to_string(),
            "mkdir" => "Create directories".to_string(),
            "touch" => "Create empty files or update timestamps".to_string(),
            "chmod" => "Change file permissions".to_string(),
            "chown" => "Change file ownership".to_string(),
            "ps" => "Show running processes".to_string(),
            "top" | "htop" => "Show system resource usage".to_string(),
            "df" => "Show disk space usage".to_string(),
            "du" => "Show directory space usage".to_string(),
            "free" => "Show memory usage".to_string(),
            "curl" => "Make HTTP requests".to_string(),
            "wget" => "Download files from the internet".to_string(),
            "git" => match cmd_parts.get(1) {
                Some(&"status") => "Show git repository status".to_string(),
                Some(&"add") => "Stage files for commit".to_string(),
                Some(&"commit") => "Create a git commit".to_string(),
                Some(&"push") => "Push changes to remote repository".to_string(),
                Some(&"pull") => "Pull changes from remote repository".to_string(),
                Some(&"clone") => "Clone a git repository".to_string(),
                _ => "Execute git command".to_string(),
            },
            "npm" | "yarn" => match cmd_parts.get(1) {
                Some(&"install") => "Install Node.js dependencies".to_string(),
                Some(&"start") => "Start the application".to_string(),
                Some(&"build") => "Build the application".to_string(),
                Some(&"test") => "Run tests".to_string(),
                _ => "Execute package manager command".to_string(),
            },
            "cargo" => match cmd_parts.get(1) {
                Some(&"build") => "Build Rust project".to_string(),
                Some(&"run") => "Run Rust project".to_string(),
                Some(&"test") => "Run Rust tests".to_string(),
                Some(&"check") => "Check Rust code".to_string(),
                _ => "Execute Cargo command".to_string(),
            },
            _ => format!("Execute command: {}", cmd_parts[0]),
        }
    }
    
    /// Executes a shell command asynchronously with a specified timeout.
    ///
    /// Runs the given command string, capturing its standard output and error, and returns a `CommandResult` containing execution details. If the command does not complete within the timeout, an error is returned.
    ///
    /// # Returns
    /// - `Ok(CommandResult)` if the command executes within the timeout.
    /// - `Err` if the command is empty, fails to execute, or times out.
    ///
    /// # Examples
    ///
    /// ```
    /// let result = execute_command_with_timeout("echo hello", 5).await?;
    /// assert!(result.success);
    /// assert_eq!(result.stdout.trim(), "hello");
    /// ```
    pub async fn execute_command_with_timeout(
        command: &str, 
        timeout_secs: u64
    ) -> Result<CommandResult> {
        let start_time = std::time::Instant::now();
        
        // Split command into parts
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Err(anyhow!("Empty command"));
        }
        
        let program = parts[0];
        let args = &parts[1..];
        
        // Create command with timeout
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            AsyncCommand::new(program)
                .args(args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
        ).await;
        
        let execution_time = start_time.elapsed();
        
        match output {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                
                Ok(CommandResult {
                    success: output.status.success(),
                    stdout,
                    stderr,
                    execution_time_ms: execution_time.as_millis() as u64,
                    exit_code: output.status.code(),
                })
            }
            Ok(Err(e)) => Err(anyhow!("Command execution failed: {}", e)),
            Err(_) => Err(anyhow!("Command timed out after {} seconds", timeout_secs)),
        }
    }
    
    /// Executes a shell command asynchronously with a 30-second timeout and returns its standard output if successful.
    ///
    /// Returns an error containing the standard error output if the command fails.
    ///
    /// # Examples
    ///
    /// ```
    /// let output = Agent::execute_command("echo hello").await.unwrap();
    /// assert_eq!(output.trim(), "hello");
    /// ```
    pub async fn execute_command(command: &str) -> Result<String> {
        let result = Self::execute_command_with_timeout(command, 30).await?;
        
        if result.success {
            Ok(result.stdout)
        } else {
            Err(anyhow!("Command failed: {}", result.stderr))
        }
    }
    
    /// Suggests relevant shell commands based on user intent and system context.
    ///
    /// Generates a list of command suggestions tailored to the user's intent keywords and the current project type, Git repository status, and common system tasks. Suggestions include project-specific build, test, install, and run commands, Git operations, system monitoring, and file search commands.
    ///
    /// # Examples
    ///
    /// ```
    /// let context = SystemContext {
    ///     project_type: ProjectType::Rust,
    ///     is_git_repo: true,
    ///     ..Default::default()
    /// };
    /// let suggestions = suggest_commands("build and test", &context);
    /// assert!(suggestions.contains(&"cargo build".to_string()));
    /// assert!(suggestions.contains(&"cargo test".to_string()));
    /// ```
    pub fn suggest_commands(intent: &str, context: &SystemContext) -> Vec<String> {
        let intent_lower = intent.to_lowercase();
        let mut suggestions = Vec::new();
        
        // Project-specific suggestions
        match context.project_type {
            ProjectType::Rust => {
                if intent_lower.contains("build") {
                    suggestions.extend(vec![
                        "cargo build".to_string(),
                        "cargo build --release".to_string(),
                        "cargo check".to_string(),
                    ]);
                }
                if intent_lower.contains("test") {
                    suggestions.extend(vec![
                        "cargo test".to_string(),
                        "cargo test --verbose".to_string(),
                        "cargo clippy".to_string(),
                    ]);
                }
            }
            ProjectType::NodeJs => {
                if intent_lower.contains("install") {
                    suggestions.extend(vec![
                        "npm install".to_string(),
                        "npm ci".to_string(),
                        "yarn install".to_string(),
                    ]);
                }
                if intent_lower.contains("run") || intent_lower.contains("start") {
                    suggestions.extend(vec![
                        "npm start".to_string(),
                        "npm run dev".to_string(),
                        "npm run build".to_string(),
                    ]);
                }
            }
            ProjectType::Python => {
                if intent_lower.contains("install") {
                    suggestions.extend(vec![
                        "pip install -r requirements.txt".to_string(),
                        "pip install --upgrade pip".to_string(),
                        "poetry install".to_string(),
                    ]);
                }
                if intent_lower.contains("run") {
                    suggestions.extend(vec![
                        "python main.py".to_string(),
                        "python -m pytest".to_string(),
                    ]);
                }
            }
            _ => {}
        }
        
        // Git suggestions
        if context.is_git_repo {
            if intent_lower.contains("status") {
                suggestions.push("git status".to_string());
            }
            if intent_lower.contains("commit") {
                suggestions.extend(vec![
                    "git add .".to_string(),
                    "git commit -m \"Update\"".to_string(),
                    "git push".to_string(),
                ]);
            }
        }
        
        // System monitoring suggestions
        if intent_lower.contains("monitor") || intent_lower.contains("resources") {
            suggestions.extend(vec![
                "htop".to_string(),
                "ps aux".to_string(),
                "df -h".to_string(),
                "free -h".to_string(),
                "lscpu".to_string(),
            ]);
        }
        
        // File operations
        if intent_lower.contains("find") || intent_lower.contains("search") {
            suggestions.extend(vec![
                "find . -name \"*.rs\"".to_string(),
                "grep -r \"pattern\" .".to_string(),
                "fd pattern".to_string(),
                "rg pattern".to_string(),
            ]);
        }
        
        suggestions
    }
    
    /// Returns a list of command completions that start with the given partial input, tailored to the project type and Git repository status.
    ///
    /// Suggests common commands relevant to the detected project type (Rust, NodeJs, Python, or generic), and includes Git commands if the current directory is a Git repository.
    ///
    /// # Examples
    ///
    /// ```
    /// let context = SystemContext {
    ///     project_type: ProjectType::Rust,
    ///     is_git_repo: true,
    ///     ..Default::default()
    /// };
    /// let completions = complete_command("git ", &context);
    /// assert!(completions.contains(&"git status".to_string()));
    /// ```
    pub fn complete_command(partial: &str, context: &SystemContext) -> Vec<String> {
        let mut completions = Vec::new();
        
        // Common command completions
        let common_commands = match context.project_type {
            ProjectType::Rust => vec![
                "cargo build", "cargo test", "cargo run", "cargo check",
                "cargo clippy", "cargo fmt", "cargo doc", "cargo clean"
            ],
            ProjectType::NodeJs => vec![
                "npm install", "npm start", "npm test", "npm run build",
                "npm run dev", "yarn install", "yarn start", "yarn build"
            ],
            ProjectType::Python => vec![
                "python", "pip install", "pip freeze", "python -m pytest",
                "python setup.py", "poetry install", "poetry run"
            ],
            _ => vec!["ls", "cd", "pwd", "cat", "grep", "find"],
        };
        
        for cmd in common_commands {
            if cmd.starts_with(partial) {
                completions.push(cmd.to_string());
            }
        }
        
        // Git completions if in git repo
        if context.is_git_repo {
            let git_commands = vec![
                "git status", "git add", "git commit", "git push", "git pull",
                "git branch", "git checkout", "git merge", "git log", "git diff"
            ];
            
            for cmd in git_commands {
                if cmd.starts_with(partial) {
                    completions.push(cmd.to_string());
                }
            }
        }
        
        completions
    }
    
    /// Analyzes a command error and provides severity, category, and recovery suggestions.
    ///
    /// Examines the error message to determine its severity and category, and generates actionable suggestions for resolving the issue. Suggestions are tailored for common error types such as permission issues, missing commands, file not found, network problems, and disk space errors. Also indicates whether retrying the command is recommended.
    ///
    /// # Examples
    ///
    /// ```
    /// let analysis = Agent::analyze_error("ls /root", "Permission denied");
    /// assert_eq!(analysis.category, ErrorCategory::Permission);
    /// assert!(analysis.suggestions.iter().any(|s| s.contains("sudo")));
    /// ```
    pub fn analyze_error(command: &str, error: &str) -> ErrorAnalysis {
        let mut suggestions = Vec::new();
        let error_lower = error.to_lowercase();
        
        // Permission errors
        if error_lower.contains("permission denied") {
            suggestions.push("Try running with sudo if appropriate".to_string());
            suggestions.push("Check file permissions with 'ls -la'".to_string());
            suggestions.push("Ensure you own the file/directory".to_string());
        }
        
        // Command not found errors
        if error_lower.contains("command not found") || error_lower.contains("not found") {
            let cmd_parts: Vec<&str> = command.split_whitespace().collect();
            if let Some(missing_cmd) = cmd_parts.first() {
                suggestions.push(format!("Install {} using your package manager", missing_cmd));
                suggestions.push(format!("Check if {} is in your PATH", missing_cmd));
                suggestions.push("Verify the command spelling".to_string());
            }
        }
        
        // File/directory not found
        if error_lower.contains("no such file or directory") {
            suggestions.push("Check if the file/directory exists with 'ls'".to_string());
            suggestions.push("Verify the path is correct".to_string());
            suggestions.push("Use absolute path instead of relative".to_string());
        }
        
        // Network errors
        if error_lower.contains("network") || error_lower.contains("connection") {
            suggestions.push("Check your internet connection".to_string());
            suggestions.push("Verify DNS resolution".to_string());
            suggestions.push("Try again later".to_string());
        }
        
        // Disk space errors
        if error_lower.contains("no space left") || error_lower.contains("disk full") {
            suggestions.push("Check disk space with 'df -h'".to_string());
            suggestions.push("Free up space by removing unnecessary files".to_string());
            suggestions.push("Check for large files with 'du -sh *'".to_string());
        }
        
        let severity = if error_lower.contains("fatal") || error_lower.contains("critical") {
            ErrorSeverity::Critical
        } else if error_lower.contains("error") {
            ErrorSeverity::High
        } else if error_lower.contains("warning") {
            ErrorSeverity::Medium
        } else {
            ErrorSeverity::Low
        };
        
        ErrorAnalysis {
            severity,
            category: Self::categorize_error(&error_lower),
            suggestions,
            retry_recommended: !error_lower.contains("permission denied") && 
                              !error_lower.contains("command not found"),
        }
    }
    
    /// Categorizes an error message into a predefined error category based on its content.
    ///
    /// Returns an `ErrorCategory` variant such as Permission, NotFound, Network, Syntax, Resource, or Other, depending on keywords found in the error string.
    ///
    /// # Examples
    ///
    /// ```
    /// let category = categorize_error("permission denied");
    /// assert_eq!(category, ErrorCategory::Permission);
    /// ```
    fn categorize_error(error: &str) -> ErrorCategory {
        if error.contains("permission") {
            ErrorCategory::Permission
        } else if error.contains("command not found") || error.contains("not found") {
            ErrorCategory::NotFound
        } else if error.contains("network") || error.contains("connection") {
            ErrorCategory::Network
        } else if error.contains("syntax") || error.contains("parse") {
            ErrorCategory::Syntax
        } else if error.contains("space") || error.contains("disk") {
            ErrorCategory::Resource
        } else {
            ErrorCategory::Other
        }
    }
    
    /// Generates a detailed, context-aware prompt for the agent to guide intelligent and safe shell command suggestions based on the user's request and current system state.
    ///
    /// The prompt includes information about the current directory, operating system, project type, Git repository status, and available tools. It outlines safety protocols and command strategies to ensure user safety and effective task completion.
    ///
    /// # Examples
    ///
    /// ```
    /// let prompt = Agent::create_agent_prompt("Set up the project and run tests", "");
    /// assert!(prompt.contains("SYSTEM CONTEXT:"));
    /// assert!(prompt.contains("USER REQUEST: Set up the project and run tests"));
    /// ```
    pub fn create_agent_prompt(user_request: &str, context: &str) -> String {
        let system_context = Self::get_system_context();
        
        let project_context = match system_context.project_type {
            ProjectType::Rust => "This is a Rust project. Prefer cargo commands for building, testing, and dependency management.",
            ProjectType::NodeJs => "This is a Node.js project. Use npm/yarn for package management and node for execution.",
            ProjectType::Python => "This is a Python project. Use pip for packages and python for execution.",
            ProjectType::Go => "This is a Go project. Use go commands for building and module management.",
            ProjectType::Generic => "This appears to be a generic project or directory.",
        };
        
        let git_context = if system_context.is_git_repo {
            "This directory is a Git repository. You can use git commands for version control operations."
        } else {
            "This is not a Git repository."
        };
        
        let tools_context = if !system_context.available_tools.is_empty() {
            format!("Available tools: {}", system_context.available_tools.join(", "))
        } else {
            "Limited tools available on this system.".to_string()
        };
        
        format!(
            r#"You are an intelligent terminal assistant with deep system knowledge. Your goal is to help users accomplish tasks efficiently and safely.

SYSTEM CONTEXT:
- Current directory: {}
- Operating System: {}
- Project type: {}
- Git repository: {}
- {}

SAFETY PROTOCOL:
1. Always prioritize user safety and data integrity
2. Suggest the safest approach first, then mention alternatives
3. Explain potential risks for destructive operations
4. Use read-only commands for exploration before modifications
5. Wrap all shell commands in ```bash code blocks

COMMAND STRATEGY:
1. Break complex tasks into logical steps
2. Provide explanatory context for each command
3. Suggest verification steps after destructive operations
4. Offer alternative approaches when applicable

USER REQUEST: {}

Please analyze this request and provide a step-by-step approach with appropriate shell commands. Explain your reasoning and highlight any potential risks."#,
            system_context.current_dir,
            system_context.os_type,
            project_context,
            git_context,
            tools_context,
            user_request
        )
    }
    
    /// Retrieves the current system context, including directory, project type, Git status, available tools, OS, and user permissions.
    ///
    /// This function gathers information about the environment to enable context-aware command suggestions and completions. It detects the current working directory, checks for the presence of a Git repository, determines the project type based on common project files, identifies available development tools, detects the operating system, and infers the user's permission level.
    ///
    /// # Returns
    /// A `SystemContext` struct containing details about the current environment.
    ///
    /// # Examples
    ///
    /// ```
    /// let context = Agent::get_system_context();
    /// assert!(!context.current_dir.is_empty());
    /// ```
    pub fn get_system_context() -> SystemContext {
        let current_dir = env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "Unknown".to_string());
        
        let is_git_repo = Path::new(".git").exists();
        
        let project_type = if Path::new("Cargo.toml").exists() {
            ProjectType::Rust
        } else if Path::new("package.json").exists() {
            ProjectType::NodeJs
        } else if Path::new("requirements.txt").exists() || Path::new("pyproject.toml").exists() {
            ProjectType::Python
        } else if Path::new("go.mod").exists() {
            ProjectType::Go
        } else {
            ProjectType::Generic
        };
        
        let os_type = env::consts::OS.to_string();
        
        // Check if user has sudo access (simplified check)
        let user_permissions = if env::var("USER").unwrap_or_default() == "root" {
            PermissionLevel::Root
        } else {
            PermissionLevel::User // We'll assume sudo availability can be checked later
        };
        
        let mut available_tools = Vec::new();
        // Check for common tools
        for tool in &["git", "cargo", "npm", "python", "docker", "curl", "wget"] {
            if Self::check_tool_available(tool) {
                available_tools.push(tool.to_string());
            }
        }
        
        SystemContext {
            current_dir,
            is_git_repo,
            project_type,
            available_tools,
            os_type,
            user_permissions,
        }
    }
    
    /// Checks if a given tool is available in the system's PATH.
    ///
    /// Returns `true` if the tool is found, otherwise `false`.
    ///
    /// # Examples
    ///
    /// ```
    /// assert_eq!(check_tool_available("ls"), true);
    /// assert_eq!(check_tool_available("nonexistent_tool_xyz"), false);
    /// ```
    fn check_tool_available(tool: &str) -> bool {
        std::process::Command::new("which")
            .arg(tool)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone)]
pub struct ErrorAnalysis {
    pub severity: ErrorSeverity,
    pub category: ErrorCategory,
    pub suggestions: Vec<String>,
    pub retry_recommended: bool,
}

#[derive(Debug, Clone)]
pub enum ErrorSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone)]
pub enum ErrorCategory {
    Permission,
    NotFound,
    Network,
    Syntax,
    Resource,
    Other,
}
