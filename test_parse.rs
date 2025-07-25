// Quick test for tool call parsing
use std::collections::HashMap;

fn parse_tool_calls(response: &str) -> Vec<(String, HashMap<String, serde_json::Value>)> {
    let mut tool_calls = Vec::new();
    
    println!("DEBUG: Starting to parse tool calls from response");
    println!("DEBUG: Response length: {} chars", response.len());
    
    // Look for tool call patterns in the response
    let lines: Vec<&str> = response.lines().collect();
    let mut i = 0;
    
    while i < lines.len() {
        let line = lines[i].trim();
        
        // Look for function call patterns like: <tool_call name="function_name">
        if line.starts_with("<tool_call") && line.contains("name=") {
            println!("DEBUG: Found tool call start: {}", line);
            if let Some(name_start) = line.find("name=\"") {
                let name_start = name_start + 6;
                if let Some(name_end) = line[name_start..].find("\"") {
                    let tool_name = &line[name_start..name_start + name_end];
                    println!("DEBUG: Extracted tool name: {}", tool_name);
                    
                    // Collect arguments until </tool_call>
                    let mut args = HashMap::new();
                    let mut content_lines = Vec::new();
                    i += 1;
                    
                    while i < lines.len() && !lines[i].trim().starts_with("</tool_call>") {
                        let arg_line = lines[i].trim();
                        println!("DEBUG: Processing line: '{}'", arg_line);
                        
                        // Check if it's a key=value pair
                        if let Some(eq_pos) = arg_line.find('=') {
                            let key = arg_line[..eq_pos].trim();
                            let value = arg_line[eq_pos + 1..].trim().trim_matches('"');
                            args.insert(key.to_string(), serde_json::Value::String(value.to_string()));
                            println!("DEBUG: Added key=value: {}={}", key, value);
                        } else if !arg_line.is_empty() {
                            // If it's not key=value, treat it as content for 'command' parameter
                            content_lines.push(arg_line);
                            println!("DEBUG: Added content line: '{}'", arg_line);
                        }
                        i += 1;
                    }
                    
                    // If we collected content lines and no explicit 'command' arg, use content as command
                    if !content_lines.is_empty() && !args.contains_key("command") {
                        let command = content_lines.join("\n");
                        args.insert("command".to_string(), serde_json::Value::String(command.clone()));
                        println!("DEBUG: Set command from content: '{}'", command);
                    }
                    
                    // Add default description if missing
                    if !args.contains_key("description") {
                        args.insert("description".to_string(), serde_json::Value::String("AI agent tool execution".to_string()));
                    }
                    
                    println!("DEBUG: Final args: {:?}", args);
                    tool_calls.push((tool_name.to_string(), args));
                }
            }
        }
        i += 1;
    }
    
    println!("DEBUG: Returning {} tool calls", tool_calls.len());
    tool_calls
}

fn main() {
    let test_response = r#"Let's analyze the Git diff to understand the changes made in the repository.

To view the Git diff, I will use the `execute_command` tool. Here is the command:

<tool_call name="execute_command">
git diff
</tool_call>

Please approve the execution of this command to display the Git diff."#;

    println!("Testing with response:");
    println!("{}", test_response);
    println!("\n" + &"=".repeat(50));
    
    let tool_calls = parse_tool_calls(test_response);
    
    println!("\nFinal result:");
    for (name, args) in tool_calls {
        println!("Tool: {}", name);
        println!("Args: {:?}", args);
    }
}
