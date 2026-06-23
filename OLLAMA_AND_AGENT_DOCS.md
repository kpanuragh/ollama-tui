# Ollama Integration & Agent Mode Documentation

## 📡 What We Send to Ollama

### Request Structure

Every chat request to Ollama sends:

```rust
pub struct ChatRequest<'a> {
    pub model: &'a str,           // e.g., "llama3", "mistral"
    pub messages: &'a [Message],  // Full conversation history
    pub stream: bool,             // Always true for streaming
}
```

### Message Structure

```rust
pub struct Message {
    pub role: Role,        // "user" or "assistant"
    pub content: String,   // The actual message text
    // Note: timestamp is NOT sent (local only, has #[serde(default)])
}
```

### Example Request Payload

```json
{
  "model": "llama3",
  "stream": true,
  "messages": [
    {
      "role": "assistant",
      "content": "New chat started. Ask me anything!"
    },
    {
      "role": "user",
      "content": "What is Rust?"
    },
    {
      "role": "assistant",
      "content": "Rust is a systems programming language..."
    },
    {
      "role": "user",
      "content": "How do I create a vector?"
    }
  ]
}
```

### Key Points

✅ **Full conversation history** is sent with each request (context maintained)
✅ **No timestamps sent** to Ollama (stored locally only)
✅ **Streaming enabled** for real-time responses
✅ **Authentication** included if configured (Bearer or Basic Auth)
❌ **No system prompts** sent (currently)
❌ **No temperature/parameters** sent (uses Ollama defaults)

### API Endpoints Used

| Endpoint | Purpose | Method |
|----------|---------|--------|
| `/api/chat` | Send messages, get responses | POST |
| `/api/tags` | List available models | GET |

---

## 🤖 How Agent Mode Works

### Overview

Agent mode allows the AI to **suggest shell commands** which you can **review and approve** before execution.

### Complete Workflow

```
┌─────────────────────────────────────────────────────────────┐
│                    AGENT MODE FLOW                           │
└─────────────────────────────────────────────────────────────┘

1. USER ENTERS AGENT MODE
   Type: :a
   ↓
   State: app.agent_mode = true
   Mode: AppMode::Agent

2. USER MAKES REQUEST
   Input: "list all rust files in src/"
   ↓
   Same as normal chat - sent to Ollama:
   {
     "model": "llama3",
     "messages": [...history, "list all rust files in src/"],
     "stream": true
   }

3. AI RESPONDS WITH COMMANDS
   AI Response:
   "Here's how to list Rust files:

   ```bash
   find src/ -name "*.rs"
   ```

   This will show all .rs files in the src directory."
   ↓
   Response streamed back and displayed in chat

4. COMMAND PARSING (Automatic)
   Location: src/main.rs:166-169
   ↓
   When streaming completes (OllamaDone event):
   if app_state.agent_mode {
       let commands = agent::Agent::parse_commands_from_response(&response);
       // Extracts: ["find src/ -name \"*.rs\""]
   }
   ↓
   Parsing logic (src/agent.rs:18-42):
   - Uses regex: r"```(?:bash|sh|shell)\s*\n([\s\S]*?)```"
   - Finds all code blocks with bash/sh/shell tags
   - Extracts each command line
   - Skips empty lines and comments

5. APPROVAL INTERFACE OPENS
   Mode: AppMode::AgentApproval
   ↓
   Display:
   ╭─ 🤖 Agent Command Approval ──────────╮
   │ ○ [PENDING] find src/ -name "*.rs"   │
   ╰──────────────────────────────────────╯
   │ j/k:nav | y:approve | x:execute     │
   └──────────────────────────────────────┘

6. USER REVIEWS & APPROVES
   Keys:
   - j/k: Navigate commands
   - y: Approve current → cmd.approved = true
   - n: Reject current → cmd.approved = false
   - a: Approve all
   - r: Reject all
   - x/Enter: Execute approved

7. COMMAND EXECUTION
   Location: src/events.rs:344-374
   ↓
   For each approved command:
   tokio::spawn(async move {
       let result = Agent::execute_command(&command).await;
       // Executes: sh -c "find src/ -name \"*.rs\""
   });
   ↓
   Execution (src/agent.rs:46-85):
   - Uses tokio::process::Command
   - Runs: sh -c "<command>"
   - Captures stdout and stderr
   - Returns Result<String, String>

8. RESULTS DISPLAYED IN CHAT
   Location: src/main.rs:218-234
   ↓
   Success:
   "Command executed successfully:
   ```
   find src/ -name "*.rs"
   ```

   Output:
   ```
   src/main.rs
   src/app.rs
   src/ui.rs
   ...
   ```"

   OR

   Failure:
   "Command failed:
   ```
   invalid_command
   ```

   Error:
   ```
   sh: invalid_command: command not found
   ```"
```

### State Management

```rust
// In AppState (src/app.rs:48-52)
pub agent_mode: bool,                          // Is agent mode active?
pub pending_commands: Vec<AgentCommand>,       // Commands to approve
pub command_approval_index: Option<usize>,     // Currently selected command
```

### Command Structure

```rust
pub struct AgentCommand {
    pub command: String,        // The shell command
    pub approved: bool,         // User approved?
    pub executed: bool,         // Already run?
    pub output: Option<String>, // Stdout/stderr if run
    pub error: Option<String>,  // Error message if failed
}
```

### Safety Features

1. **Explicit Approval Required**
   - Commands NEVER execute automatically
   - User must press 'y' to approve each command
   - Can approve all with 'a' or reject all with 'r'

2. **Visual Review**
   - Full command displayed before execution
   - Clear status indicators (○ pending, ✓ approved)
   - Can cancel anytime with ESC

3. **Sandboxed Execution**
   - Runs in user's shell context (sh -c)
   - No elevated privileges
   - Standard shell restrictions apply

4. **Error Handling**
   - Non-zero exit codes captured
   - Stderr displayed in chat
   - Execution failures don't crash app

### Command Parsing Details

**Recognized formats:**
```bash
# WORKS ✓
```bash
ls -la
```

```sh
pwd
```

```shell
echo "hello"
```

# DOESN'T WORK ✗
Plain text without code blocks
`inline code` (backticks, not triple backticks)
```python  (wrong language tag)
echo "test"
```
```

**Parsing rules:**
- Extracts lines from code blocks
- Skips empty lines
- Skips comments (lines starting with #)
- Each line becomes a separate command

**Example:**
```bash
# This is a comment (skipped)
ls -la           # Command 1

pwd              # Command 2
# Another comment (skipped)
```

Results in 2 commands: `["ls -la", "pwd"]`

### Event Flow

```rust
// Event types (src/events.rs:9-18)
pub enum AppEvent {
    Terminal(KeyEvent),                      // User keyboard input
    OllamaChunk(Result<String, String>),     // AI response chunk
    OllamaDone,                              // AI finished responding
    Models(Result<Vec<String>, String>),     // Model list fetched
    AgentCommands(Vec<AgentCommand>),        // Commands parsed from AI
    CommandExecuted(usize, Result<...>),     // Command execution result
    Tick,                                    // UI update timer
}
```

**Event sequence:**
1. User presses Enter → `Terminal(KeyEvent)`
2. Ollama streams response → Multiple `OllamaChunk` events
3. Streaming completes → `OllamaDone` event
4. Commands parsed → `AgentCommands` event (switches to approval mode)
5. User approves & executes → Multiple `CommandExecuted` events

### Differences from Normal Chat

| Aspect | Normal Chat | Agent Mode |
|--------|-------------|------------|
| Input handling | Same | Same |
| Ollama request | Standard | **Same** (no special prompt) |
| Response display | Display only | Display + Parse commands |
| After response | Done | Open approval UI |
| Command execution | N/A | User-approved execution |
| Results | N/A | Added back to chat |

**Key insight:** Agent mode doesn't change what we send to Ollama. It just adds post-processing of the response to extract and execute commands.

### Example Session

**Step 1: Enter agent mode**
```
User: :a
Status: "🤖 Agent Mode | ESC→normal | Enter→send | Commands will need approval"
```

**Step 2: Make request**
```
User: "Show me disk usage of src directory"
```

**Step 3: AI response**
```
AI: "Here's how to check disk usage:

```bash
du -sh src/
du -h src/* | sort -h
```

The first command shows total size, the second shows individual files sorted by size."
```

**Step 4: Approval interface**
```
╭─ 🤖 Agent Command Approval ───────────────────╮
│ ▶ ○ [PENDING] du -sh src/                    │
│   ○ [PENDING] du -h src/* | sort -h          │
╰───────────────────────────────────────────────╯
```

**Step 5: User approves both**
```
Press 'a' (approve all)

╭─ 🤖 Agent Command Approval ───────────────────╮
│ ▶ ✓ [APPROVED] du -sh src/                   │
│   ✓ [APPROVED] du -h src/* | sort -h         │
╰───────────────────────────────────────────────╯
```

**Step 6: Execute**
```
Press 'x' (execute)

Chat shows:
"Command executed successfully:
```
du -sh src/
```

Output:
```
456K    src/
```"

"Command executed successfully:
```
du -h src/* | sort -h
```

Output:
```
12K     src/agent.rs
24K     src/app.rs
...
```"
```

---

## 🔍 Technical Details

### Code Locations

| Functionality | File | Lines |
|---------------|------|-------|
| Ollama API calls | `src/ollama.rs` | 39-118 |
| Chat request struct | `src/models.rs` | 162-167 |
| Agent parsing | `src/agent.rs` | 18-42 |
| Agent execution | `src/agent.rs` | 46-85 |
| Command approval UI | `src/events.rs` | 254-374 |
| Approval rendering | `src/ui.rs` | 412-512 |
| Command results | `src/main.rs` | 213-237 |

### Performance Characteristics

**Ollama requests:**
- ✅ Streaming for low latency
- ✅ Full history for context
- ⚠️ History grows with conversation (could be large)

**Agent mode:**
- ✅ Command parsing is fast (regex)
- ✅ Async execution doesn't block UI
- ⚠️ Long-running commands block until complete
- ⚠️ No timeout (commands can hang)

### Limitations

**Current agent mode limitations:**

1. **No System Prompt**
   - AI not specifically instructed to format commands
   - Relies on AI naturally using code blocks
   - May miss commands in plain text

2. **No Context About System**
   - AI doesn't know your OS, shell, or environment
   - May suggest incompatible commands
   - No working directory info sent

3. **Single Shell Execution**
   - Each command runs independently (no state)
   - Can't do: `cd /tmp && ls` effectively
   - No environment persistence

4. **No Streaming Command Output**
   - Must wait for command completion
   - Long-running commands appear frozen
   - Can't cancel running commands

5. **Limited Error Context**
   - Just shows stdout/stderr
   - No exit code in display
   - No command timing info

---

## 🚀 Potential Enhancements

### Short Term
- [ ] Add system prompt for agent mode
- [ ] Include OS/shell info in context
- [ ] Command timeout (e.g., 30 seconds)
- [ ] Exit code display

### Medium Term
- [ ] Streaming command output
- [ ] Working directory selection
- [ ] Environment variable support
- [ ] Command cancellation
- [ ] Multi-command sessions (persistent shell)

### Long Term
- [ ] Sandboxed execution environment
- [ ] Command risk assessment (dangerous command detection)
- [ ] Execution history log
- [ ] Remote execution support
- [ ] Plugin system for custom commands

---

## 🎯 Summary

### What Goes to Ollama
- **Model name**
- **Full message history** (role + content)
- **Stream flag** (always true)
- **Authentication** (if configured)

### What Stays Local
- **Timestamps** (display only)
- **Command approval state**
- **Execution results** (until added back to chat)
- **UI state** (modes, selections, etc.)

### How Agent Works
1. **Normal chat** with Ollama (no special handling)
2. **Parse response** for code blocks with bash/sh/shell
3. **Show approval UI** with extracted commands
4. **User approves** which commands to run
5. **Execute** via `sh -c "<command>"`
6. **Display results** back in chat

**Key Philosophy:** Agent mode is a post-processing layer that extracts and executes commands from AI responses, not a special AI mode. The AI doesn't "know" it's in agent mode.
