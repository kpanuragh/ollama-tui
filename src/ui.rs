use crate::{
    app::{AppMode, AppState},
    models,
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    layout::Rect,
};
use textwrap::wrap;
use unicode_width::UnicodeWidthStr;
use chrono::Timelike;

#[allow(dead_code)]
pub fn get_chat_area(f_area: Rect) -> Rect {
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)].as_ref())
        .split(f_area);

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(4), Constraint::Length(2)].as_ref())
        .split(main_chunks[0]);
    left_chunks[0]
}

pub fn ui(f: &mut Frame, app: &mut AppState) {
    // Use modern 70/30 split instead of 75/25 for better balance
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)].as_ref())
        .split(f.area());

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(4), Constraint::Length(2)].as_ref())
        .split(main_chunks[0]);

    // Modern gradient-like border styling
    let chat_border_style = Style::default()
        .fg(app.config.theme.parse_color(&app.config.theme.chat_border_color))
        .add_modifier(Modifier::BOLD);

    let sessions_border_style = if app.mode == AppMode::SessionSelection {
        Style::default()
            .fg(Color::LightCyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(app.config.theme.parse_color(&app.config.theme.sessions_border_color))
    };

    // Modern session panel title with icons
    let sessions_title = if app.mode == AppMode::SessionSelection {
        format!("📋 Sessions ({}/{}) ⌨",
            app.current_session_index + 1,
            app.sessions.len())
    } else {
        format!("📋 Sessions ({}/{})",
            app.current_session_index + 1,
            app.sessions.len())
    };

    // Create the list items first, before borrowing app mutably
    let messages = app.current_messages().clone();
    let theme = app.config.theme.clone();
    let visual_selection = if app.mode == AppMode::Visual {
        app.visual_start.zip(app.visual_end).map(|(start, end)| (start.min(end), start.max(end)))
    } else {
        None
    };
    let chat_list_items = render_modern_messages(&messages, left_chunks[0].width, &theme, visual_selection);

    // Modern chat title with model info and status
    let chat_title = format!(
        "💬 Chat - {} {} {}",
        app.current_model,
        if app.is_loading { "🔄" } else { "" },
        if app.agent_mode { "🤖" } else { "" }
    );

    let chat_list = List::new(chat_list_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(chat_title)
                .title_style(Style::default().add_modifier(Modifier::BOLD))
                .border_style(chat_border_style)
                .border_type(ratatui::widgets::BorderType::Rounded),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(40, 40, 60))
                .add_modifier(Modifier::BOLD)
        )
        .highlight_symbol("▶ ");
    f.render_stateful_widget(chat_list, left_chunks[0], &mut app.chat_list_state);

    // Modern mode indicator with better styling
    let (mode_text, mode_style, mode_icon) = match app.mode {
        AppMode::Normal => ("NORMAL", Style::default().fg(Color::LightGreen), "🟢"),
        AppMode::Insert => ("INSERT", Style::default().fg(Color::LightBlue), "✏️"),
        AppMode::Command => ("COMMAND", Style::default().fg(Color::LightYellow), "⚡"),
        AppMode::Visual => ("VISUAL", Style::default().fg(Color::LightMagenta), "👁️"),
        AppMode::ModelSelection => ("MODEL", Style::default().fg(Color::LightCyan), "🤖"),
        AppMode::SessionSelection => ("SESSION", Style::default().fg(Color::LightCyan), "📋"),
        AppMode::Agent => ("AGENT", Style::default().fg(Color::LightRed), "🤖"),
        AppMode::AgentApproval => ("APPROVE", Style::default().fg(Color::LightYellow), "✅"),
        AppMode::Help => ("HELP", Style::default().fg(Color::White), "❓"),
    };

    let input_title = format!("{} {} {}", mode_icon, mode_text, mode_icon);

    let input_text = match app.mode {
        AppMode::Command => format!(":{}", app.vim_command),
        _ => {
            if app.is_loading {
                "🔄 Thinking...".to_string()
            } else {
                app.input.clone()
            }
        }
    };

    // Modern input box with better styling
    let input_paragraph = Paragraph::new(input_text.as_str())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(input_title)
                .title_style(mode_style.add_modifier(Modifier::BOLD))
                .border_type(ratatui::widgets::BorderType::Rounded)
                .border_style(mode_style)
        )
        .wrap(Wrap { trim: false });
    f.render_widget(input_paragraph, left_chunks[1]);

    // Enhanced status bar with more information and better formatting
    let status_bar_text = if let Some(ref msg) = app.status_message {
        format!("ℹ️  {}", msg)
    } else {
        match app.mode {
            AppMode::Normal => {
                let session_info = format!("Session {}/{}", app.current_session_index + 1, app.sessions.len());
                let msg_count = app.current_messages().len();
                format!(
                    "📊 {} | 💬 {} msgs | 🤖 {} | ⌨ ? help | i insert | v visual | :q quit",
                    session_info, msg_count, app.current_model
                )
            },
            AppMode::Insert => format!(
                "📝 Typing... | ESC→normal | Enter→send | Model: {}",
                app.current_model
            ),
            AppMode::Agent => format!(
                "🤖 Agent Mode | ESC→normal | Enter→send | Commands will need approval"
            ),
            AppMode::Command => "⚡ Type command and press Enter | ESC to cancel".to_string(),
            AppMode::Visual => "👁️ VISUAL | j/k extend | y copy | ESC exit".to_string(),
            AppMode::SessionSelection => "📋 SESSION | j/k navigate | Enter select | d delete | ESC exit".to_string(),
            AppMode::AgentApproval => "✅ APPROVE | j/k navigate | y approve | n reject | a all | x execute".to_string(),
            _ => format!("Model: {} | ESC→normal", app.current_model),
        }
    };

    let status_bar = Paragraph::new(status_bar_text)
        .style(
            Style::default()
                .fg(app.config.theme.parse_color(&app.config.theme.status_bar_color))
                .bg(Color::Rgb(20, 20, 30))
        )
        .wrap(Wrap { trim: false });
    f.render_widget(status_bar, left_chunks[2]);

    // Set cursor position based on mode
    match app.mode {
        AppMode::Insert => {
            if !app.is_loading {
                let cursor_x = left_chunks[1].x + app.input.width() as u16 + 1;
                let cursor_y = left_chunks[1].y + 1 + (app.input.len() as u16 / (left_chunks[1].width.saturating_sub(2)));
                f.set_cursor_position((cursor_x.min(left_chunks[1].right().saturating_sub(2)), cursor_y.min(left_chunks[1].bottom().saturating_sub(2))));
            }
        }
        AppMode::Agent => {
            if !app.is_loading {
                let cursor_x = left_chunks[1].x + app.input.width() as u16 + 1;
                let cursor_y = left_chunks[1].y + 1;
                f.set_cursor_position((cursor_x.min(left_chunks[1].right().saturating_sub(2)), cursor_y.min(left_chunks[1].bottom().saturating_sub(2))));
            }
        }
        AppMode::Command => {
            f.set_cursor_position((
                left_chunks[1].x + app.vim_command.width() as u16 + 2, // +2 for ":"
                left_chunks[1].y + 1,
            ));
        }
        _ => {}
    }

    // Modern session list with better visual indicators
    let session_items: Vec<ListItem> = app
        .sessions
        .iter()
        .enumerate()
        .map(|(idx, s)| {
            let is_current = s.id == app.sessions[app.current_session_index].id;
            let icon = if is_current { "▶" } else { " " };
            let msg_count = s.messages.len();

            let style = if is_current {
                Style::default()
                    .fg(Color::LightCyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };

            let session_text = format!("{} {} ({} msgs)", icon, s.name, msg_count);
            ListItem::new(session_text).style(style)
        })
        .collect();

    let sessions_highlight_style = if app.mode == AppMode::SessionSelection {
        Style::default()
            .bg(Color::LightCyan)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .bg(Color::Rgb(40, 40, 60))
            .fg(Color::White)
    };

    let sessions_list = List::new(session_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(sessions_title)
                .title_style(Style::default().add_modifier(Modifier::BOLD))
                .border_style(sessions_border_style)
                .border_type(ratatui::widgets::BorderType::Rounded),
        )
        .highlight_style(sessions_highlight_style)
        .highlight_symbol("▶ ");

    f.render_stateful_widget(sessions_list, main_chunks[1], &mut app.session_list_state);

    if app.mode == AppMode::ModelSelection {
        render_model_selection_popup(f, app);
    }

    if app.mode == AppMode::AgentApproval {
        render_modern_agent_approval_popup(f, app);
    }

    if app.mode == AppMode::Help {
        render_help_popup(f, app);
    }
}

fn render_model_selection_popup(f: &mut Frame, app: &mut AppState) {
    let popup_area = centered_rect(60, 50, f.area());
    let block = Block::default()
        .title("🤖 Select Model")
        .title_style(Style::default().add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(Style::default().fg(app.config.theme.parse_color(&app.config.theme.popup_border_color)));

    if app.is_fetching_models {
        let text = Paragraph::new("🔄 Fetching models...")
            .alignment(Alignment::Center)
            .block(block);
        f.render_widget(Clear, popup_area);
        f.render_widget(text, popup_area);
        return;
    }

    if app.available_models.is_empty() {
        let text = Paragraph::new(
            "⚠️  No models found\n\nEnsure Ollama is running and models are pulled.\n\nPress 'q' to close.",
        )
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .block(block);
        f.render_widget(Clear, popup_area);
        f.render_widget(text, popup_area);
        return;
    }

    let items: Vec<ListItem> = app
        .available_models
        .iter()
        .map(|model| {
            let is_current = model == &app.current_model;
            let icon = if is_current { "●" } else { "○" };
            ListItem::new(format!(" {} {}", icon, model))
                .style(if is_current {
                    Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                })
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(40, 60, 100))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_widget(Clear, popup_area);
    f.render_stateful_widget(list, popup_area, &mut app.model_list_state);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

// Modern message rendering with timestamps and better styling
fn render_modern_messages<'a>(
    messages: &'a [models::Message],
    width: u16,
    theme: &crate::models::Theme,
    visual_selection: Option<(usize, usize)>
) -> Vec<ListItem<'a>> {
    let mut list_items = Vec::new();
    let mut line_index = 0;

    for (msg_idx, message) in messages.iter().enumerate() {
        let (role_style, role_icon, role_name) = match message.role {
            models::Role::User => (
                Style::default().fg(theme.parse_color(&theme.user_message_color)),
                "👤",
                "You"
            ),
            models::Role::Assistant => (
                Style::default().fg(theme.parse_color(&theme.assistant_message_color)),
                "🤖",
                "AI"
            ),
        };

        // Format timestamp as HH:MM
        let time_str = format!(
            "{:02}:{:02}",
            message.timestamp.hour(),
            message.timestamp.minute()
        );

        // Header line with icon, role, and timestamp
        let header = format!("{} {} • {}", role_icon, role_name, time_str);
        let header_style = role_style.add_modifier(Modifier::BOLD);

        list_items.push(ListItem::new(Line::from(vec![
            Span::styled(header, header_style),
        ])));
        line_index += 1;

        // Message content with proper wrapping
        let wrapped_content = wrap(&message.content, (width as usize).saturating_sub(4));

        for line_content in wrapped_content.iter() {
            let line_style = if let Some((start, end)) = visual_selection {
                if line_index >= start && line_index <= end {
                    role_style.bg(Color::Blue).add_modifier(Modifier::REVERSED)
                } else {
                    role_style
                }
            } else {
                role_style
            };

            let line = Line::from(vec![
                Span::raw("  "),
                Span::styled(line_content.to_string(), line_style),
            ]);
            list_items.push(ListItem::new(line));
            line_index += 1;
        }

        // Add spacing between messages (except after the last one)
        if msg_idx < messages.len() - 1 {
            list_items.push(ListItem::new(Line::from("")));
            line_index += 1;
        }
    }

    list_items
}

// Modern agent approval popup with enhanced visuals
fn render_modern_agent_approval_popup(f: &mut Frame, app: &AppState) {
    let popup_area = centered_rect(85, 75, f.area());

    let block = Block::default()
        .title("🤖 Agent Command Approval")
        .title_style(Style::default().add_modifier(Modifier::BOLD).fg(Color::LightCyan))
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(Style::default().fg(Color::LightYellow));

    if app.pending_commands.is_empty() {
        let text = Paragraph::new("ℹ️  No commands to approve")
            .alignment(Alignment::Center)
            .block(block);
        f.render_widget(Clear, popup_area);
        f.render_widget(text, popup_area);
        return;
    }

    // Create command list items with rich formatting
    let items: Vec<ListItem> = app
        .pending_commands
        .iter()
        .enumerate()
        .map(|(i, cmd)| {
            let (status_icon, status_text) = if cmd.executed {
                if cmd.error.is_some() {
                    ("❌", "FAILED")
                } else {
                    ("✅", "DONE")
                }
            } else if cmd.approved {
                ("✓", "APPROVED")
            } else {
                ("○", "PENDING")
            };

            let mut style = if cmd.executed {
                if cmd.error.is_some() {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().fg(Color::Green).add_modifier(Modifier::DIM)
                }
            } else if cmd.approved {
                Style::default().fg(Color::LightGreen)
            } else {
                Style::default().fg(Color::Gray)
            };

            // Highlight current selection
            if Some(i) == app.command_approval_index {
                style = style.add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
            }

            let command_display = if cmd.command.len() > 80 {
                format!("{}...", &cmd.command[..77])
            } else {
                cmd.command.clone()
            };

            ListItem::new(format!(
                " {} [{}] {}",
                status_icon,
                status_text,
                command_display
            )).style(style)
        })
        .collect();

    // Create help text
    let help_text = " ⌨  j/k:navigate | y:approve | n:reject | a:all | r:none | x/Enter:execute | ESC:cancel ";
    let help_style = Style::default().bg(Color::Rgb(40, 40, 60)).fg(Color::White);

    // Split popup into command list and help bar
    let popup_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(popup_area);

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(60, 60, 100))
                .add_modifier(Modifier::BOLD)
        )
        .highlight_symbol("▶ ");

    let help_bar = Paragraph::new(help_text)
        .style(help_style)
        .alignment(Alignment::Center);

    f.render_widget(Clear, popup_area);

    // Create a ListState for rendering
    let mut list_state = ratatui::widgets::ListState::default();
    list_state.select(app.command_approval_index);

    f.render_stateful_widget(list, popup_chunks[0], &mut list_state);
    f.render_widget(help_bar, popup_chunks[1]);
}

fn render_help_popup(f: &mut Frame, app: &mut AppState) {
    let popup_area = centered_rect(80, 85, f.area());
    let block = Block::default()
        .title("❓ Ollama TUI - Help Guide")
        .title_style(Style::default().add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(Style::default().fg(Color::LightCyan));

    let help_text = vec![
        "=== 🎮 NORMAL MODE KEYS ===",
        "  i              - Enter insert mode (type messages)",
        "  o/O            - Enter insert mode (clear input first)",
        "  v              - Enter visual mode (select text to copy)",
        "  :              - Enter command mode",
        "  ?              - Show this help",
        "  q              - Quick quit",
        "  j/k or ↑/↓     - Scroll chat up/down",
        "  g              - Go to top of chat",
        "  G              - Go to bottom of chat",
        "  PgUp/PgDn      - Page up/down through chat",
        "",
        "=== ✏️  INSERT MODE KEYS ===",
        "  ESC            - Return to normal mode",
        "  Enter          - Send message",
        "  Backspace      - Delete character",
        "  *Any character - Type message",
        "",
        "=== 👁️  VISUAL MODE KEYS ===",
        "  j/k or ↑/↓     - Extend selection",
        "  g              - Go to top",
        "  G              - Go to bottom",
        "  PgUp/PgDn      - Page up/down",
        "  y              - Copy selection to clipboard",
        "  ESC/q          - Return to normal mode",
        "",
        "=== ⚡ COMMAND MODE COMMANDS ===",
        "  :q             - Quit application",
        "  :w             - Save current session",
        "  :wq            - Save and quit",
        "  :n             - Create new session",
        "  :c             - Clear current session",
        "  :m             - Select model",
        "  :s             - Select session",
        "  :a             - Enter agent mode (with history)",
        "  :an            - Enter agent mode (fresh, no history)",
        "  :h or :?       - Show this help",
        "  :d             - Delete current session",
        "  :d<N>          - Delete session N",
        "  :b<N>          - Switch to session N",
        "",
        "=== 🤖 AGENT MODE ===",
        "  :a   - Agent mode with conversation history",
        "  :an  - Agent mode with fresh context (no previous messages)",
        "  AI suggests shell commands based on your requests",
        "  Commands are parsed from code blocks",
        "  Review and approve before execution",
        "",
        "=== ✅ AGENT APPROVAL MODE KEYS ===",
        "  j/k or ↑/↓     - Navigate commands",
        "  y              - Approve current command",
        "  n              - Reject current command",
        "  a              - Approve all commands",
        "  r              - Reject all commands",
        "  x or Enter     - Execute approved commands",
        "  ESC or q       - Cancel and return to agent mode",
        "",
        "=== 📋 SPECIAL MODES ===",
        "  Model Selection - Use j/k or ↑/↓ to navigate, Enter to select",
        "  Session Selection - Use j/k or ↑/↓ to navigate, Enter to select, d to delete",
        "",
        "Press ESC or q to close this help",
    ];

    let help_paragraph = Paragraph::new(help_text.join("\n"))
        .block(block)
        .wrap(Wrap { trim: true })
        .scroll((0, 0));

    f.render_widget(Clear, popup_area);
    f.render_widget(help_paragraph, popup_area);
}
