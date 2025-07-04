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

pub fn get_chat_area(f_area: Rect) -> Rect {
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(75), Constraint::Percentage(25)].as_ref())
        .split(f_area);

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3), Constraint::Length(1)].as_ref())
        .split(main_chunks[0]);
    left_chunks[0]
}

pub fn ui(f: &mut Frame, app: &mut AppState) {

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(75), Constraint::Percentage(25)].as_ref())
        .split(f.size());

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3), Constraint::Length(1)].as_ref())
        .split(main_chunks[0]);

    let chat_border_style = Style::default().fg(app.config.theme.parse_color(&app.config.theme.chat_border_color));
    let sessions_border_style = if app.mode == AppMode::SessionSelection {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(app.config.theme.parse_color(&app.config.theme.sessions_border_color))
    };

    let sessions_title = if app.mode == AppMode::SessionSelection {
        format!("Sessions ({}/{}) [j/k:navigate | Enter:select | d:delete | ESC:exit]", 
            app.current_session_index + 1, 
            app.sessions.len())
    } else {
        format!("Sessions ({}/{}) [:n | :s | :d]", 
            app.current_session_index + 1, 
            app.sessions.len())
    };

    // Create the list items first, before borrowing app mutably
    let messages = app.current_messages().clone();
    let theme = app.config.theme.clone();
    let chat_list_items = render_messages_as_list(&messages, left_chunks[0].width, &theme);
    
    let chat_list = List::new(chat_list_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Chat History (↑↓ to scroll, PgUp/PgDn to page)")
                .border_style(chat_border_style),
        )
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::DIM))
        .highlight_symbol("  ");  // Less intrusive highlight
    f.render_stateful_widget(chat_list, left_chunks[0], &mut app.chat_list_state);

    let input_title = match app.mode {
        AppMode::Normal => "-- NORMAL --",
        AppMode::Insert => "-- INSERT --",
        AppMode::Command => "-- COMMAND --",
        AppMode::ModelSelection => "-- MODEL SELECTION --",
        AppMode::SessionSelection => "-- SESSION SELECTION --",
        AppMode::Agent => "-- AGENT --",
        AppMode::Help => "-- HELP --",
    };

    let input_text = match app.mode {
        AppMode::Command => format!(":{}", app.vim_command),
        _ => {
            if app.is_loading {
                "Thinking...".to_string()
            } else {
                app.input.clone()
            }
        }
    };
    
    let input_paragraph = Paragraph::new(input_text.as_str())
        .block(Block::default().borders(Borders::ALL).title(input_title));
    f.render_widget(input_paragraph, left_chunks[1]);

    let status_bar_text = match app.mode {
        AppMode::Normal => format!(
            "Model: {} | ? for help | i:insert | :q quit | :n new | :m models | :s sessions",
            app.current_model
        ),
        AppMode::Insert => format!(
            "Model: {} | ESC to normal mode | Enter to send",
            app.current_model
        ),
        AppMode::Command => "Type command and press Enter".to_string(),
        AppMode::SessionSelection => "SESSION SELECTION: j/k to navigate | Enter to select | d to delete | ESC to exit".to_string(),
        _ => format!("Model: {} | ESC to normal mode", app.current_model),
    };
    let status_bar = Paragraph::new(status_bar_text).style(Style::default().fg(app.config.theme.parse_color(&app.config.theme.status_bar_color)));
    f.render_widget(status_bar, left_chunks[2]);

    // Set cursor position based on mode
    match app.mode {
        AppMode::Insert => {
            if !app.is_loading {
                f.set_cursor(
                    left_chunks[1].x + app.input.width() as u16 + 1,
                    left_chunks[1].y + 1,
                );
            }
        }
        AppMode::Command => {
            f.set_cursor(
                left_chunks[1].x + app.vim_command.width() as u16 + 2, // +2 for ":"
                left_chunks[1].y + 1,
            );
        }
        _ => {}
    }

    let session_items: Vec<ListItem> = app
        .sessions
        .iter()
        .map(|s| {
            let style = if s.id == app.sessions[app.current_session_index].id {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(s.name.as_str()).style(style)
        })
        .collect();

    let sessions_highlight_style = if app.mode == AppMode::SessionSelection {
        Style::default()
            .bg(Color::Yellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .bg(app.config.theme.parse_color(&app.config.theme.highlight_bg_color))
            .fg(app.config.theme.parse_color(&app.config.theme.highlight_color))
    };

    let sessions_list = List::new(session_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(sessions_title)
                .border_style(sessions_border_style),
        )
        .highlight_style(sessions_highlight_style)
        .highlight_symbol(">> ");

    f.render_stateful_widget(sessions_list, main_chunks[1], &mut app.session_list_state);

    if app.mode == AppMode::ModelSelection {
        render_model_selection_popup(f, app);
    }
    
    if app.mode == AppMode::Help {
        render_help_popup(f, app);
    }
}

fn render_model_selection_popup(f: &mut Frame, app: &mut AppState) {
    let popup_area = centered_rect(60, 50, f.size());
    let block = Block::default()
        .title("Select a Model (Enter to confirm, Esc/q to cancel)")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.config.theme.parse_color(&app.config.theme.popup_border_color)));

    if app.is_fetching_models {
        let text = Paragraph::new("Fetching models...")
            .alignment(Alignment::Center)
            .block(block);
        f.render_widget(Clear, popup_area);
        f.render_widget(text, popup_area);
        return;
    }

    if app.available_models.is_empty() {
        let text = Paragraph::new(
            "No models found. Ensure Ollama is running and models are pulled. Press 'q' to close.",
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
        .map(|s| ListItem::new(s.as_str()))
        .collect();
    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(app.config.theme.parse_color(&app.config.theme.highlight_bg_color))
                .fg(app.config.theme.parse_color(&app.config.theme.highlight_color))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

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

fn render_messages<'a>(messages: &'a [models::Message], width: u16, theme: &crate::models::Theme) -> Text<'a> {
    let mut lines = Vec::new();
    for message in messages {
        let style = match message.role {
            models::Role::User => Style::default().fg(theme.parse_color(&theme.user_message_color)),
            models::Role::Assistant => Style::default().fg(theme.parse_color(&theme.assistant_message_color)),
        };
        let prefix = match message.role {
            models::Role::User => "You: ",
            models::Role::Assistant => "AI: ",
        };
        let wrapped_content = wrap(&message.content, (width as usize).saturating_sub(6));
        for (i, line_content) in wrapped_content.iter().enumerate() {
            if i == 0 {
                lines.push(Line::from(vec![
                    Span::styled(prefix, style.add_modifier(Modifier::BOLD)),
                    Span::styled(line_content.to_string(), style),
                ]));
            } else {
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(line_content.to_string(), style),
                ]));
            }
        }
        if !message.content.is_empty() {
            lines.push(Line::from(""));
        }
    }
    Text::from(lines)
}

fn render_messages_as_list<'a>(messages: &'a [models::Message], width: u16, theme: &crate::models::Theme) -> Vec<ListItem<'a>> {
    let mut list_items = Vec::new();
    
    for message in messages {
        let style = match message.role {
            models::Role::User => Style::default().fg(theme.parse_color(&theme.user_message_color)),
            models::Role::Assistant => Style::default().fg(theme.parse_color(&theme.assistant_message_color)),
        };
        let prefix = match message.role {
            models::Role::User => "You: ",
            models::Role::Assistant => "AI: ",
        };
        
        let wrapped_content = wrap(&message.content, (width as usize).saturating_sub(6));
        
        for (i, line_content) in wrapped_content.iter().enumerate() {
            if i == 0 {
                // First line with prefix
                let line = Line::from(vec![
                    Span::styled(prefix, style.add_modifier(Modifier::BOLD)),
                    Span::styled(line_content.to_string(), style),
                ]);
                list_items.push(ListItem::new(line));
            } else {
                // Continuation lines with indentation
                let line = Line::from(vec![
                    Span::raw("    "),
                    Span::styled(line_content.to_string(), style),
                ]);
                list_items.push(ListItem::new(line));
            }
        }
        
        // Add empty line after each message if content is not empty
        if !message.content.is_empty() {
            list_items.push(ListItem::new(Line::from("")));
        }
    }
    
    list_items
}

fn render_help_popup(f: &mut Frame, app: &mut AppState) {
    let popup_area = centered_rect(80, 70, f.area());
    let block = Block::default()
        .title("Help - Vim-style Commands (Press ? or ESC to close)")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.config.theme.parse_color(&app.config.theme.popup_border_color)));

    let help_text = vec![
        "VIM-STYLE NAVIGATION AND COMMANDS",
        "",
        "MODES:",
        "  Normal Mode    - Navigate and issue commands",
        "  Insert Mode    - Type messages",
        "  Command Mode   - Enter vim-style commands",
        "",
        "NORMAL MODE KEYS:",
        "  i              - Enter insert mode",
        "  o/O            - Enter insert mode (clear input)",
        "  :              - Enter command mode",
        "  ?              - Show this help",
        "  q              - Quick quit",
        "  j/↓            - Scroll down",
        "  k/↑            - Scroll up",
        "  g              - Go to top",
        "  G              - Go to bottom",
        "  PgUp/PgDn      - Page up/down",
        "",
        "INSERT MODE KEYS:",
        "  ESC            - Return to normal mode",
        "  Enter          - Send message",
        "  Backspace      - Delete character",
        "",
        "COMMAND MODE COMMANDS:",
        "  :q             - Quit application",
        "  :w             - Save current session",
        "  :wq            - Save and quit",
        "  :n             - Create new session",
        "  :c             - Clear current session",
        "  :m             - Select model",
        "  :s             - Select session",
        "  :a             - Enter agent mode",
        "  :h or :?       - Show this help",
        "  :d             - Delete current session",
        "  :d<N>          - Delete session N",
        "  :b<N>          - Switch to session N",
        "",
        "SPECIAL MODES:",
        "  Model Selection - Use j/k or ↑/↓ to navigate, Enter to select",
        "  Session Selection - Use j/k or ↑/↓ to navigate, Enter to select, d to delete",
        "  Agent Mode     - Interactive AI agent (experimental)",
    ];

    let help_paragraph = Paragraph::new(help_text.join("\n"))
        .block(block)
        .wrap(Wrap { trim: true })
        .scroll((0, 0));

    f.render_widget(Clear, popup_area);
    f.render_widget(help_paragraph, popup_area);
}

