# Agent Mode: Fresh Context vs. History

## Overview

Agent mode now offers two ways to start:

| Command | Name | Description |
|---------|------|-------------|
| **`:a`** | Agent with history | Starts agent mode with full conversation history |
| **`:an`** | Agent fresh/new | Starts agent mode with clean slate (no previous messages) |

---

## `:a` - Agent with History

### When to Use
- ✅ Continuing from previous conversation
- ✅ Agent needs context from earlier commands
- ✅ Building on previous work
- ✅ Multi-step tasks that reference prior steps

### What Happens
```
User: :a
→ Enters agent mode
→ Keeps ALL previous messages
→ AI has full conversation context
→ System prompt includes current context (OS, directory, git, etc.)
```

### Example Session
```
[Previous messages: "What files are here?", "Show me logs", etc.]

User: :a
User: "Now analyze those log files we just found"
AI: *remembers previous conversation about log files*
```

### Advantages
- ✅ AI remembers what you discussed
- ✅ Can reference previous commands
- ✅ Maintains conversation flow
- ✅ Better for multi-step workflows

### Disadvantages
- ⚠️ Long history can slow down responses
- ⚠️ Old context may influence new suggestions
- ⚠️ More tokens = higher API cost (if applicable)
- ⚠️ Conversation clutter

---

## `:an` - Agent with Fresh Context

### When to Use
- ✅ Starting a completely new task
- ✅ Previous conversation is too long
- ✅ Old context is confusing the AI
- ✅ Want faster responses
- ✅ Previous messages aren't relevant

### What Happens
```
User: :an
→ Clears ALL previous messages from session
→ Adds fresh start message
→ Enters agent mode
→ System prompt includes current context (OS, directory, git, etc.)
→ AI has no knowledge of previous conversation
```

### Example Session
```
[Previous messages: Long conversation about logs, configs, etc.]

User: :an
Assistant: "Agent mode started with fresh context. I can suggest shell commands to help you."

User: "Find large files in the current directory"
AI: *no knowledge of previous conversation, focuses only on this task*
```

### Advantages
- ✅ Clean slate for new tasks
- ✅ Faster AI responses (less context)
- ✅ No interference from old conversation
- ✅ Lower token usage
- ✅ Fresh perspective

### Disadvantages
- ⚠️ Loses all previous context
- ⚠️ Can't reference earlier work
- ⚠️ Need to re-establish context if needed

---

## System Context (Always Included)

**Both `:a` and `:an` include current system context:**

```rust
SystemContext {
    current_dir: "/home/user/project",
    is_git_repo: true,
    git_branch: Some("main"),
    shell: "/bin/bash",
    os: "Linux",
    home_dir: Some("/home/user"),
}
```

This means the AI **always** knows:
- ✅ Your operating system
- ✅ Current directory
- ✅ Shell environment
- ✅ Git repository status
- ✅ Current git branch (if in repo)

**The difference is conversation history, not system context!**

---

## Comparison Table

| Feature | `:a` (History) | `:an` (Fresh) |
|---------|---------------|---------------|
| **Previous messages** | ✅ Kept | ❌ Cleared |
| **System context** | ✅ Included | ✅ Included |
| **Response speed** | Slower (more context) | Faster (less context) |
| **Token usage** | Higher | Lower |
| **Conversation continuity** | ✅ Yes | ❌ No |
| **Fresh perspective** | ❌ No | ✅ Yes |
| **Multi-step tasks** | ✅ Better | ⚠️ Limited |
| **Isolated tasks** | ⚠️ May have interference | ✅ Better |

---

## Use Cases

### Use `:a` (with history) for:

**1. Multi-step workflows**
```
:a
"Find all log files"
→ AI suggests: find . -name "*.log"

"Now grep for errors in those files"
→ AI remembers previous command, suggests grep on results
```

**2. Iterative refinement**
```
:a
"List files by size"
→ AI suggests: ls -lh

"Sort by size, largest first"
→ AI refines previous command: ls -lhS
```

**3. Building on context**
```
:a
"What git branches do we have?"
→ AI suggests: git branch -a

"Checkout the development branch"
→ AI knows we're in git repo: git checkout development
```

### Use `:an` (fresh) for:

**1. New, unrelated task**
```
[After long conversation about database backups...]

:an
"Find large files in current directory"
→ Fresh start, no database context to confuse AI
```

**2. Cluttered history**
```
[After 50+ messages with failed commands, errors, etc.]

:an
"List running processes"
→ Clean slate, AI not influenced by previous errors
```

**3. Performance optimization**
```
:an
"Quick task: check disk usage"
→ Faster response with minimal context
```

**4. Context reset**
```
:a
"Deploy to production"
→ AI suggests dangerous production commands

:an  ← Reset context
"Test in development environment"
→ AI fresh, not influenced by production context
```

---

## Behind the Scenes

### `:a` Implementation
```rust
"a" => {
    self.mode = AppMode::Agent;
    self.agent_mode = true;
    let context = Agent::gather_system_context();
    self.agent_system_prompt = Agent::create_agent_system_prompt(&context);
    // Messages remain intact
}
```

### `:an` Implementation
```rust
"an" => {
    // Clear messages
    self.current_messages_mut().clear();
    self.current_messages_mut().push(Message {
        role: Role::Assistant,
        content: "Agent mode started with fresh context...".to_string(),
        timestamp: Utc::now(),
    });

    // Enter agent mode
    self.mode = AppMode::Agent;
    self.agent_mode = true;
    let context = Agent::gather_system_context();
    self.agent_system_prompt = Agent::create_agent_system_prompt(&context);

    // Reset UI state
    self.chat_list_state = ListState::default();
    self.auto_scroll = true;
}
```

---

## Request Comparison

### With `:a` (History)
```json
{
  "model": "llama3",
  "system": "You are in AGENT MODE...\nOS: Linux\nDir: /home/user/project\n...",
  "messages": [
    {"role": "assistant", "content": "New chat started..."},
    {"role": "user", "content": "list files"},
    {"role": "assistant", "content": "Here's how: ls -la"},
    {"role": "user", "content": "find large ones"}  ← Current request
  ]
}
```
**Total context:** 4 messages + system prompt

### With `:an` (Fresh)
```json
{
  "model": "llama3",
  "system": "You are in AGENT MODE...\nOS: Linux\nDir: /home/user/project\n...",
  "messages": [
    {"role": "assistant", "content": "Agent mode started with fresh context..."},
    {"role": "user", "content": "find large files"}  ← Current request
  ]
}
```
**Total context:** 2 messages + system prompt

---

## Tips

### 💡 Best Practices

1. **Start with `:an` for isolated tasks**
   ```
   :an
   "Quick: show disk usage"
   ```

2. **Use `:a` for workflows**
   ```
   :a
   "Find config files"
   "Edit the main config"
   "Validate the syntax"
   ```

3. **Switch between them as needed**
   ```
   :a     ← Multi-step task
   ...
   :an    ← Fresh start for new task
   ```

4. **Reset when confused**
   ```
   :a
   [AI giving wrong suggestions based on old context]

   :an    ← Fresh perspective
   ```

### ⚡ Performance

- Use `:an` for **speed** (less context = faster responses)
- Use `:a` for **accuracy** (full context = better understanding)

### 🎯 Accuracy

- Use `:an` to **avoid confusion** from old context
- Use `:a` to **maintain continuity** in multi-step tasks

---

## FAQ

**Q: Does `:an` delete messages from database?**
A: No, it only clears the current in-memory session. Database is unchanged. Messages are still saved.

**Q: Can I get my history back after `:an`?**
A: No, once cleared for the session, history is gone. But you can switch sessions (`:s`) or create new session (`:n`) to keep your original session intact.

**Q: Does `:an` affect system context?**
A: No, system context (OS, directory, git) is always gathered fresh regardless of `:a` or `:an`.

**Q: Which is default when entering from normal mode?**
A: There is no default. You must explicitly use `:a` or `:an` to enter agent mode.

**Q: Can I clear history without entering agent mode?**
A: Yes, use `:c` to clear current session (works in any mode).

**Q: Does `:an` create a new session?**
A: No, it clears messages in the CURRENT session. Use `:n` to create a new session.

---

## Keyboard Shortcuts Summary

```
:a    → Agent mode with history
:an   → Agent mode fresh (no history)
:c    → Clear current session (any mode)
:n    → New session
ESC   → Exit agent mode
```

---

## Summary

**`:a`** = Agent with full conversation **history**
- Good for: Multi-step tasks, building on previous work
- Trade-off: Slower, more context

**`:an`** = Agent with **fresh** context
- Good for: New tasks, speed, clean slate
- Trade-off: No conversation memory

**Both include current system context** (OS, directory, git, etc.)

Choose based on whether you need conversation continuity or a fresh start!
