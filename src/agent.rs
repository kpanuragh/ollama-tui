use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::process::Command as TokioCommand;
use std::process::Command;
use chrono::Utc;
use std::env;

/// Represents a tool function that can be called by the agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: ToolParameters,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameters {
    #[serde(rename = "type")]
    pub param_type: String,
    pub properties: HashMap<String, ToolProperty>,
    pub required: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolProperty {
    #[serde(rename = "type")]
    pub prop_type: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<ToolProperty>>,
}

/// Tool execution result
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
}

/// Enum containing all available tools
#[derive(Clone, Debug)]
pub enum ToolInstance {
    ReadFile(ReadFileTool),
    WriteFile(WriteFileTool),
    ListDirectory(ListDirectoryTool),
    ExecuteCommand(ExecuteCommandTool),
    SearchFiles(SearchFilesTool),
    GetWorkingDirectory(GetWorkingDirectoryTool),
}

impl ToolInstance {
    pub fn name(&self) -> &str {
        match self {
            ToolInstance::ReadFile(tool) => tool.name(),
            ToolInstance::WriteFile(tool) => tool.name(),
            ToolInstance::ListDirectory(tool) => tool.name(),
            ToolInstance::ExecuteCommand(tool) => tool.name(),
            ToolInstance::SearchFiles(tool) => tool.name(),
            ToolInstance::GetWorkingDirectory(tool) => tool.name(),
        }
    }

    pub fn description(&self) -> &str {
        match self {
            ToolInstance::ReadFile(tool) => tool.description(),
            ToolInstance::WriteFile(tool) => tool.description(),
            ToolInstance::ListDirectory(tool) => tool.description(),
            ToolInstance::ExecuteCommand(tool) => tool.description(),
            ToolInstance::SearchFiles(tool) => tool.description(),
            ToolInstance::GetWorkingDirectory(tool) => tool.description(),
        }
    }

    pub fn get_function_definition(&self) -> ToolFunction {
        match self {
            ToolInstance::ReadFile(tool) => tool.get_function_definition(),
            ToolInstance::WriteFile(tool) => tool.get_function_definition(),
            ToolInstance::ListDirectory(tool) => tool.get_function_definition(),
            ToolInstance::ExecuteCommand(tool) => tool.get_function_definition(),
            ToolInstance::SearchFiles(tool) => tool.get_function_definition(),
            ToolInstance::GetWorkingDirectory(tool) => tool.get_function_definition(),
        }
    }

    pub async fn execute(&self, args: &HashMap<String, serde_json::Value>) -> Result<ToolResult> {
        match self {
            ToolInstance::ReadFile(tool) => tool.execute(args).await,
            ToolInstance::WriteFile(tool) => tool.execute(args).await,
            ToolInstance::ListDirectory(tool) => tool.execute(args).await,
            ToolInstance::ExecuteCommand(tool) => tool.execute(args).await,
            ToolInstance::SearchFiles(tool) => tool.execute(args).await,
            ToolInstance::GetWorkingDirectory(tool) => tool.execute(args).await,
        }
    }

    pub fn requires_approval(&self) -> bool {
        match self {
            ToolInstance::ReadFile(tool) => tool.requires_approval(),
            ToolInstance::WriteFile(tool) => tool.requires_approval(),
            ToolInstance::ListDirectory(tool) => tool.requires_approval(),
            ToolInstance::ExecuteCommand(tool) => tool.requires_approval(),
            ToolInstance::SearchFiles(tool) => tool.requires_approval(),
            ToolInstance::GetWorkingDirectory(tool) => tool.requires_approval(),
        }
    }
}

/// Agent system that manages tool execution and LLM interaction
#[derive(Clone)]
pub struct Agent {
    tools: HashMap<String, ToolInstance>,
    system_prompt: String,
    current_directory: String,
    os_info: String,
    shell_info: String,
    git_status: String,
    fixed_execution_context: Option<String>,
}

impl Agent {
    pub fn new() -> Result<Self> {
        let current_directory = env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "/".to_string());

        let os_info = format!("{} {}", env::consts::OS, env::consts::ARCH);
        let shell_info = env::var("SHELL").unwrap_or_else(|_| "unknown".into());
        let git_info = Self::get_git_info();
        let _system_info = Self::get_system_info();

        let mut tools = HashMap::new();
        
        // Register built-in tools
        tools.insert("read_file".to_string(), ToolInstance::ReadFile(ReadFileTool));
        tools.insert("write_file".to_string(), ToolInstance::WriteFile(WriteFileTool));
        tools.insert("list_directory".to_string(), ToolInstance::ListDirectory(ListDirectoryTool));
        tools.insert("execute_command".to_string(), ToolInstance::ExecuteCommand(ExecuteCommandTool));
        tools.insert("search_files".to_string(), ToolInstance::SearchFiles(SearchFilesTool));
        tools.insert("get_working_directory".to_string(), ToolInstance::GetWorkingDirectory(GetWorkingDirectoryTool));

        Ok(Self {
            tools,
            system_prompt: Self::default_system_prompt(),
            current_directory,
            os_info,
            shell_info,
            git_status: git_info,
            fixed_execution_context: None,
        })
    }

    fn default_system_prompt() -> String {
        r#"You are an AI assistant with access to system tools. You can help users with file operations, directory navigation, command execution, and other system tasks.

IMPORTANT: You must use the available tools to interact with the system. Do not provide generic advice - actually perform the requested operations using the tools.

Available capabilities:
- Read and write files
- List directory contents
- Execute shell commands (with user approval)
- Search for files and content
- Navigate the file system

When a user asks you to perform a task:
1. Use the appropriate tools to gather information
2. Execute the necessary operations
3. Provide clear feedback about what was done

Always request approval before executing potentially destructive commands or commands that might modify the system."#.to_string()
    }

    pub fn register_tool(&mut self, name: String, tool: ToolInstance) -> Result<()> {
        self.tools.insert(name, tool);
        Ok(())
    }

    pub fn get_tools_for_llm(&self) -> Vec<ToolFunction> {
        self.tools.values().map(|tool| tool.get_function_definition()).collect()
    }

    pub fn get_system_prompt(&self) -> &str {
        &self.system_prompt
    }

    pub async fn execute_tool(&self, tool_name: &str, args: &HashMap<String, serde_json::Value>) -> Result<ToolResult> {
        let tool = self.tools.get(tool_name)
            .ok_or_else(|| anyhow!("Tool '{}' not found", tool_name))?;
        
        tool.execute(args).await
    }

    pub fn tool_requires_approval(&self, tool_name: &str) -> bool {
        self.tools.get(tool_name)
            .map(|tool| tool.requires_approval())
            .unwrap_or(true)
    }

    /// Parse tool calls from LLM response
    pub fn parse_tool_calls(response: &str) -> Vec<(String, HashMap<String, serde_json::Value>)> {
        let mut tool_calls = Vec::new();
        
        // Look for tool call patterns in the response
        let lines: Vec<&str> = response.lines().collect();
        let mut i = 0;
        
        while i < lines.len() {
            let line = lines[i].trim();
            
            // Look for function call patterns - handle both formats:
            // <tool_call name="function_name"> and <tool_ call name="function_name">
            let is_tool_call = (line.starts_with("<tool_call") || line.starts_with("<tool_ call")) && line.contains("name=");
            
            if is_tool_call {
                if let Some(name_start) = line.find("name=\"") {
                    let name_start = name_start + 6;
                    if let Some(name_end) = line[name_start..].find("\"") {
                        let tool_name = &line[name_start..name_start + name_end];
                        
                        // Collect arguments until </tool_call> or </tool_ call>
                        let mut args = HashMap::new();
                        let mut content_lines = Vec::new();
                        i += 1;
                        
                        while i < lines.len() {
                            let current_line = lines[i].trim();
                            
                            // Check for end tag (both formats)
                            if current_line.starts_with("</tool_call>") || current_line.starts_with("</tool_ call>") {
                                break;
                            }
                            
                            // Check if it's a key=value pair
                            if let Some(eq_pos) = current_line.find('=') {
                                let key = current_line[..eq_pos].trim();
                                let value = current_line[eq_pos + 1..].trim().trim_matches('"');
                                args.insert(key.to_string(), serde_json::Value::String(value.to_string()));
                            } else if !current_line.is_empty() {
                                // If it's not key=value, treat it as content for 'command' parameter
                                content_lines.push(current_line);
                            }
                            i += 1;
                        }
                        
                        // If we collected content lines and no explicit 'command' arg, use content as command
                        if !content_lines.is_empty() && !args.contains_key("command") {
                            let command = content_lines.join("\n");
                            args.insert("command".to_string(), serde_json::Value::String(command));
                        }
                        
                        // Add default description if missing
                        if !args.contains_key("description") {
                            args.insert("description".to_string(), serde_json::Value::String("AI agent tool execution".to_string()));
                        }
                        
                        tool_calls.push((tool_name.to_string(), args));
                    }
                }
            }
            i += 1;
        }
        
        tool_calls
    }

    /// Get comprehensive git repository information
    fn get_git_info() -> String {
        let mut git_info = Vec::new();
        
        // Check if we're in a git repository
        if let Ok(output) = Command::new("git")
            .args(["rev-parse", "--is-inside-work-tree"])
            .output()
        {
            if !output.status.success() {
                return "Not a git repository".to_string();
            }
        } else {
            return "Git not available".to_string();
        }
        
        // Get current branch
        if let Ok(output) = Command::new("git")
            .args(["branch", "--show-current"])
            .output()
        {
            if output.status.success() {
                let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
                git_info.push(format!("Branch: {}", branch));
            }
        }
        
        // Get repository status
        if let Ok(output) = Command::new("git")
            .args(["status", "--porcelain"])
            .output()
        {
            if output.status.success() {
                let status = String::from_utf8_lossy(&output.stdout);
                if status.trim().is_empty() {
                    git_info.push("Status: Clean working tree".to_string());
                } else {
                    let lines: Vec<&str> = status.lines().collect();
                    git_info.push(format!("Status: {} changes", lines.len()));
                }
            }
        }
        
        // Get remote information
        if let Ok(output) = Command::new("git")
            .args(["remote", "-v"])
            .output()
        {
            if output.status.success() {
                let remotes = String::from_utf8_lossy(&output.stdout);
                if !remotes.trim().is_empty() {
                    let remote_lines: Vec<&str> = remotes.lines().collect();
                    git_info.push(format!("Remotes: {} configured", remote_lines.len() / 2));
                }
            }
        }
        
        // Get last commit info
        if let Ok(output) = Command::new("git")
            .args(["log", "-1", "--oneline"])
            .output()
        {
            if output.status.success() {
                let commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
                git_info.push(format!("Last commit: {}", 
                    if commit.len() > 50 { 
                        format!("{}...", &commit[..50]) 
                    } else { 
                        commit 
                    }
                ));
            }
        }
        
        if git_info.is_empty() {
            "Git repository (no additional info)".to_string()
        } else {
            git_info.join(", ")
        }
    }
    
    /// Get additional system information
    fn get_system_info() -> HashMap<String, String> {
        let mut info = HashMap::new();
        
        // Get current time
        let now = Utc::now();
        info.insert("current_time".to_string(), now.format("%Y-%m-%dT%H:%M:%SZ").to_string());
        
        // Get user information
        if let Ok(user) = env::var("USER") {
            info.insert("user".to_string(), user);
        }
        
        // Get hostname
        if let Ok(output) = Command::new("hostname").output() {
            if output.status.success() {
                let hostname = String::from_utf8_lossy(&output.stdout).trim().to_string();
                info.insert("hostname".to_string(), hostname);
            }
        }
        
        // Get shell version if possible
        if let Ok(shell_path) = env::var("SHELL") {
            if let Ok(output) = Command::new(&shell_path).arg("--version").output() {
                if output.status.success() {
                    let version = String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .next()
                        .unwrap_or("")
                        .to_string();
                    info.insert("shell_version".to_string(), version);
                }
            }
        }
        
        info
    }
    
    /// Get current execution context as JSON-like structure
    pub fn get_execution_context(&self) -> String {
        // Return the fixed context if set, otherwise generate dynamically
        if let Some(fixed_context) = &self.fixed_execution_context {
            return fixed_context.clone();
        }
        
        let system_info = Self::get_system_info();
        let current_time = system_info.get("current_time")
            .unwrap_or(&"unknown".to_string()).clone();
        let user = system_info.get("user")
            .unwrap_or(&"unknown".to_string()).clone();
        let hostname = system_info.get("hostname")
            .unwrap_or(&"unknown".to_string()).clone();
        let shell_version = system_info.get("shell_version")
            .unwrap_or(&"unknown".to_string()).clone();
        
        format!(
            r#"{{
  "execution_context": {{
    "directory_state": {{
      "pwd": "{}",
      "home": "{}"
    }},
    "operating_system": {{
      "platform": "{}",
      "architecture": "{}"
    }},
    "current_time": "{}",
    "shell": {{
      "name": "{}",
      "version": "{}"
    }},
    "user_info": {{
      "username": "{}",
      "hostname": "{}"
    }},
    "git_repository": {{
      "info": "{}"
    }}
  }}
}}"#,
            self.current_directory,
            env::var("HOME").unwrap_or_else(|_| "unknown".to_string()),
            env::consts::OS,
            env::consts::ARCH,
            current_time,
            self.shell_info.split('/').last().unwrap_or("unknown"),
            shell_version,
            user,
            hostname,
            self.git_status.replace('"', "\\\"")
        )
    }
    
    /// Set a fixed execution context that will be used instead of dynamic generation
    pub fn set_fixed_execution_context(&mut self, context: String) {
        self.fixed_execution_context = Some(context);
    }
    
    /// Clear the fixed execution context and return to dynamic generation
    pub fn clear_fixed_execution_context(&mut self) {
        self.fixed_execution_context = None;
    }
    
    /// Check if a fixed execution context is currently set
    pub fn has_fixed_execution_context(&self) -> bool {
        self.fixed_execution_context.is_some()
    }

    /// Create an enhanced prompt for agent mode
    pub fn create_agent_prompt(&self, user_input: &str) -> String {
        let tools_description = self.tools.values()
            .map(|tool| {
                let func_def = tool.get_function_definition();
                let params: Vec<String> = func_def.parameters.required.iter()
                    .map(|param| {
                        if let Some(prop) = func_def.parameters.properties.get(param) {
                            format!("  - {}: {} ({})", param, prop.prop_type, prop.description)
                        } else {
                            format!("  - {}: required parameter", param)
                        }
                    })
                    .collect();
                
                format!("- {}\n  Description: {}\n  Parameters:\n{}", 
                    tool.name(), 
                    tool.description(),
                    if params.is_empty() { "    (none)".to_string() } else { params.join("\n") }
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        // Get comprehensive execution context
        let execution_context = self.get_execution_context();

        format!(
            r#"{}

=== EXECUTION CONTEXT ===
{}

=== ENVIRONMENT DETAILS ===
- Working Directory: {}
- Operating System: {}
- Shell: {}
- Git Repository: {}

=== AVAILABLE TOOLS ===
{}

=== USER REQUEST ===
{}

=== TOOL USAGE FORMAT ===
IMPORTANT: To use a tool, you MUST use this exact format:
<tool_call name="tool_name">
parameter1=value1
parameter2=value2
</tool_call>

Example usage:
<tool_call name="read_file">
path=/home/user/document.txt
</tool_call>

<tool_call name="execute_command">
command=ls -la
description=List directory contents
</tool_call>

Please analyze the user's request within the context of the current environment and use the appropriate tools to help them. Be specific and practical in your approach."#,
            self.system_prompt,
            execution_context,
            self.current_directory,
            self.os_info,
            self.shell_info.split('/').last().unwrap_or("unknown"),
            self.git_status,
            tools_description,
            user_input
        )
    }
}

/// Trait that all tools must implement
pub trait Tool {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn get_function_definition(&self) -> ToolFunction;
    async fn execute(&self, args: &HashMap<String, serde_json::Value>) -> Result<ToolResult>;
    fn requires_approval(&self) -> bool {
        true // Most tools should require approval by default
    }
}

// Built-in Tools Implementation

#[derive(Clone, Debug)]
struct ReadFileTool;

impl Tool for ReadFileTool {
    fn name(&self) -> &str { "read_file" }
    
    fn description(&self) -> &str { 
        "Read the contents of a file. Use this to examine file contents before making changes." 
    }
    
    fn get_function_definition(&self) -> ToolFunction {
        let mut properties = HashMap::new();
        properties.insert("path".to_string(), ToolProperty {
            prop_type: "string".to_string(),
            description: "Path to the file to read".to_string(),
            items: None,
        });
        
        ToolFunction {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: ToolParameters {
                param_type: "object".to_string(),
                properties,
                required: vec!["path".to_string()],
            },
        }
    }
    
    async fn execute(&self, args: &HashMap<String, serde_json::Value>) -> Result<ToolResult> {
        let path = args.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing or invalid 'path' parameter"))?;
        
        match std::fs::read_to_string(path) {
            Ok(content) => Ok(ToolResult {
                success: true,
                output: content,
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to read file: {}", e)),
            }),
        }
    }
    
    fn requires_approval(&self) -> bool { false }
}

#[derive(Clone, Debug)]
struct WriteFileTool;

impl Tool for WriteFileTool {
    fn name(&self) -> &str { "write_file" }
    
    fn description(&self) -> &str { 
        "Write content to a file. This will create the file if it doesn't exist or overwrite if it does." 
    }
    
    fn get_function_definition(&self) -> ToolFunction {
        let mut properties = HashMap::new();
        properties.insert("path".to_string(), ToolProperty {
            prop_type: "string".to_string(),
            description: "Path to the file to write".to_string(),
            items: None,
        });
        properties.insert("content".to_string(), ToolProperty {
            prop_type: "string".to_string(),
            description: "Content to write to the file".to_string(),
            items: None,
        });
        
        ToolFunction {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: ToolParameters {
                param_type: "object".to_string(),
                properties,
                required: vec!["path".to_string(), "content".to_string()],
            },
        }
    }
    
    async fn execute(&self, args: &HashMap<String, serde_json::Value>) -> Result<ToolResult> {
        let path = args.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing or invalid 'path' parameter"))?;
        
        let content = args.get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing or invalid 'content' parameter"))?;
        
        match std::fs::write(path, content) {
            Ok(_) => Ok(ToolResult {
                success: true,
                output: format!("Successfully wrote {} bytes to {}", content.len(), path),
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to write file: {}", e)),
            }),
        }
    }
}

#[derive(Clone, Debug)]
struct ListDirectoryTool;

impl Tool for ListDirectoryTool {
    fn name(&self) -> &str { "list_directory" }
    
    fn description(&self) -> &str { 
        "List the contents of a directory. Shows files and subdirectories." 
    }
    
    fn get_function_definition(&self) -> ToolFunction {
        let mut properties = HashMap::new();
        properties.insert("path".to_string(), ToolProperty {
            prop_type: "string".to_string(),
            description: "Path to the directory to list (default: current directory)".to_string(),
            items: None,
        });
        
        ToolFunction {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: ToolParameters {
                param_type: "object".to_string(),
                properties,
                required: vec![],
            },
        }
    }
    
    async fn execute(&self, args: &HashMap<String, serde_json::Value>) -> Result<ToolResult> {
        let path = args.get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");
        
        match std::fs::read_dir(path) {
            Ok(entries) => {
                let mut items = Vec::new();
                for entry in entries {
                    if let Ok(entry) = entry {
                        let name = entry.file_name().to_string_lossy().to_string();
                        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
                        items.push(if is_dir { format!("{}/", name) } else { name });
                    }
                }
                items.sort();
                
                Ok(ToolResult {
                    success: true,
                    output: items.join("\n"),
                    error: None,
                })
            },
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to list directory: {}", e)),
            }),
        }
    }
    
    fn requires_approval(&self) -> bool { false }
}

#[derive(Clone, Debug)]
struct ExecuteCommandTool;

impl Tool for ExecuteCommandTool {
    fn name(&self) -> &str { "execute_command" }
    
    fn description(&self) -> &str { 
        "Execute a shell command. Use with caution - requires user approval." 
    }
    
    fn get_function_definition(&self) -> ToolFunction {
        let mut properties = HashMap::new();
        properties.insert("command".to_string(), ToolProperty {
            prop_type: "string".to_string(),
            description: "The shell command to execute".to_string(),
            items: None,
        });
        properties.insert("description".to_string(), ToolProperty {
            prop_type: "string".to_string(),
            description: "Brief description of what this command does".to_string(),
            items: None,
        });
        
        ToolFunction {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: ToolParameters {
                param_type: "object".to_string(),
                properties,
                required: vec!["command".to_string(), "description".to_string()],
            },
        }
    }
    
    async fn execute(&self, args: &HashMap<String, serde_json::Value>) -> Result<ToolResult> {
        let command = args.get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing or invalid 'command' parameter"))?;
        
        match TokioCommand::new("sh")
            .arg("-c")
            .arg(command)
            .output()
            .await
        {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                
                // Limit output length to prevent UI issues
                const MAX_OUTPUT_LENGTH: usize = 2000;
                let truncated_stdout = if stdout.len() > MAX_OUTPUT_LENGTH {
                    format!("{}... [output truncated, {} total chars]", 
                            &stdout[..MAX_OUTPUT_LENGTH], stdout.len())
                } else {
                    stdout.to_string()
                };
                
                let truncated_stderr = if stderr.len() > MAX_OUTPUT_LENGTH {
                    format!("{}... [output truncated, {} total chars]", 
                            &stderr[..MAX_OUTPUT_LENGTH], stderr.len())
                } else {
                    stderr.to_string()
                };
                
                if output.status.success() {
                    Ok(ToolResult {
                        success: true,
                        output: truncated_stdout,
                        error: if stderr.is_empty() { None } else { Some(truncated_stderr) },
                    })
                } else {
                    Ok(ToolResult {
                        success: false,
                        output: truncated_stdout,
                        error: Some(truncated_stderr),
                    })
                }
            },
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to execute command: {}", e)),
            }),
        }
    }
}

#[derive(Clone, Debug)]
struct SearchFilesTool;

impl Tool for SearchFilesTool {
    fn name(&self) -> &str { "search_files" }
    
    fn description(&self) -> &str { 
        "Search for files by name pattern or search for text content within files." 
    }
    
    fn get_function_definition(&self) -> ToolFunction {
        let mut properties = HashMap::new();
        properties.insert("pattern".to_string(), ToolProperty {
            prop_type: "string".to_string(),
            description: "Search pattern (file name pattern or text to search for)".to_string(),
            items: None,
        });
        properties.insert("search_type".to_string(), ToolProperty {
            prop_type: "string".to_string(),
            description: "Type of search: 'filename' or 'content'".to_string(),
            items: None,
        });
        properties.insert("directory".to_string(), ToolProperty {
            prop_type: "string".to_string(),
            description: "Directory to search in (default: current directory)".to_string(),
            items: None,
        });
        
        ToolFunction {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: ToolParameters {
                param_type: "object".to_string(),
                properties,
                required: vec!["pattern".to_string(), "search_type".to_string()],
            },
        }
    }
    
    async fn execute(&self, args: &HashMap<String, serde_json::Value>) -> Result<ToolResult> {
        let pattern = args.get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing or invalid 'pattern' parameter"))?;
        
        let search_type = args.get("search_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing or invalid 'search_type' parameter"))?;
        
        let directory = args.get("directory")
            .and_then(|v| v.as_str())
            .unwrap_or(".");
        
        match search_type {
            "filename" => {
                let output = TokioCommand::new("find")
                    .arg(directory)
                    .arg("-name")
                    .arg(pattern)
                    .output()
                    .await;
                
                match output {
                    Ok(output) => Ok(ToolResult {
                        success: output.status.success(),
                        output: String::from_utf8_lossy(&output.stdout).to_string(),
                        error: if output.stderr.is_empty() { None } else { Some(String::from_utf8_lossy(&output.stderr).to_string()) },
                    }),
                    Err(e) => Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Search failed: {}", e)),
                    }),
                }
            },
            "content" => {
                let output = TokioCommand::new("grep")
                    .arg("-r")
                    .arg("-n")
                    .arg(pattern)
                    .arg(directory)
                    .output()
                    .await;
                
                match output {
                    Ok(output) => Ok(ToolResult {
                        success: output.status.success(),
                        output: String::from_utf8_lossy(&output.stdout).to_string(),
                        error: if output.stderr.is_empty() { None } else { Some(String::from_utf8_lossy(&output.stderr).to_string()) },
                    }),
                    Err(e) => Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Search failed: {}", e)),
                    }),
                }
            },
            _ => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Invalid search_type. Use 'filename' or 'content'".to_string()),
            }),
        }
    }
    
    fn requires_approval(&self) -> bool { false }
}

#[derive(Clone, Debug)]
struct GetWorkingDirectoryTool;

impl Tool for GetWorkingDirectoryTool {
    fn name(&self) -> &str { "get_working_directory" }
    
    fn description(&self) -> &str { 
        "Get the current working directory path." 
    }
    
    fn get_function_definition(&self) -> ToolFunction {
        ToolFunction {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: ToolParameters {
                param_type: "object".to_string(),
                properties: HashMap::new(),
                required: vec![],
            },
        }
    }
    
    async fn execute(&self, _args: &HashMap<String, serde_json::Value>) -> Result<ToolResult> {
        match std::env::current_dir() {
            Ok(path) => Ok(ToolResult {
                success: true,
                output: path.to_string_lossy().to_string(),
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to get working directory: {}", e)),
            }),
        }
    }
    
    fn requires_approval(&self) -> bool { false }
}
