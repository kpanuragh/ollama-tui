use crate::{
    app::{AppMode, AppState},
    models,
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};
use textwrap::wrap;
use unicode_width::UnicodeWidthStr;

/// Renders the main terminal user interface for the chat application.
///
/// This function draws the chat history, input box, status bar, and session list,
/// adapting the layout and content based on the current application mode (Normal, Agent, or ModelSelection).
/// It conditionally displays popups for model selection and agent command approval when appropriate,
/// and manages cursor positioning and visual styling according to the active theme and state.
pub fn ui(f: &mut Frame, app: &mut AppState) {

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(75), Constraint::Percentage(25)].as_ref())
        .split(f.area());

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3), Constraint::Length(1)].as_ref())
        .split(main_chunks[0]);

    let chat_border_style = Style::default().fg(app.config.theme.parse_color(&app.config.theme.chat_border_color));
    let sessions_border_style = Style::default().fg(app.config.theme.parse_color(&app.config.theme.sessions_border_color));

    let chat_messages = render_messages(app.current_messages(), left_chunks[0].width, &app.config.theme);
    let chat_paragraph = Paragraph::new(chat_messages)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Chat History")
                .border_style(chat_border_style),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_offset, 0));
    f.render_widget(chat_paragraph, left_chunks[0]);

    let input_text = if app.is_loading {
        "Thinking...".to_string()
    } else if app.mode == AppMode::Agent {
        format!("[AGENT] {}", app.input)
    } else {
        app.input.clone()
    };
    let input_paragraph = Paragraph::new(input_text.as_str())
        .block(Block::default().borders(Borders::ALL).title("Input"));
    f.render_widget(input_paragraph, left_chunks[1]);

    let status_bar_text = if app.mode == AppMode::Agent {
        format!(
            "AGENT MODE | Model: {} | Ctrl+A: Exit Agent | Y: Approve | N: Reject | Ctrl+C: Quit",
            app.current_model
        )
    } else {
        format!(
            "Model: {} | Ctrl+D: Clear | Ctrl+L: Models | Ctrl+A: Agent | Tab: Sessions | Ctrl+C: Quit",
            app.current_model
        )
    };
    let status_bar = Paragraph::new(status_bar_text).style(Style::default().fg(app.config.theme.parse_color(&app.config.theme.status_bar_color)));
    f.render_widget(status_bar, left_chunks[2]);

    if !app.is_loading && (app.mode == AppMode::Normal || app.mode == AppMode::Agent) {
        let cursor_offset = if app.mode == AppMode::Agent { 8 } else { 0 }; // Account for "[AGENT] " prefix
        f.set_cursor_position((
            left_chunks[1].x + app.input.width() as u16 + 1 + cursor_offset,
            left_chunks[1].y + 1,
        ));
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

    let sessions_list = List::new(session_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Sessions (Ctrl+N)")
                .border_style(sessions_border_style),
        )
        .highlight_style(Style::default()
            .bg(app.config.theme.parse_color(&app.config.theme.highlight_bg_color))
            .fg(app.config.theme.parse_color(&app.config.theme.highlight_color)))
        .highlight_symbol(">> ");

    f.render_stateful_widget(sessions_list, main_chunks[1], &mut app.session_list_state);

    if app.mode == AppMode::ModelSelection {
        render_model_selection_popup(f, app);
    }

    if app.mode == AppMode::Agent && app.command_approval_index.is_some() {
        render_agent_commands_popup(f, app);
    }
    
    if app.mode == AppMode::Agent && !app.pending_commands.is_empty() {
        render_agent_commands_popup(f, app);
    }
}

/// Renders a centered popup for selecting a model in the terminal UI.
///
/// The popup displays a loading message while models are being fetched, a message if no models are available,
/// or a selectable list of available models. The currently selected model is highlighted, and the user can confirm
/// or cancel the selection using keyboard controls. The popup occupies 60% of the terminal width and 50% of the height.
fn render_model_selection_popup(f: &mut Frame, app: &mut AppState) {
    let popup_area = centered_rect(60, 50, f.area());
    let block = Block::default()
        .title("Select a Model (Enter to confirm, Esc/q to cancel)")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.config.theme.parse_color(&app.config.theme.popup_border_color)));

    f.render_widget(Clear, popup_area);
    f.render_widget(block, popup_area);

    let inner_area = popup_area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });

    if app.is_fetching_models {
        let loading_text = Paragraph::new("Fetching models from Ollama...");
        f.render_widget(loading_text, inner_area);
    } else if app.available_models.is_empty() {
        let no_models_text = Paragraph::new("No models available. Is Ollama running?");
        f.render_widget(no_models_text, inner_area);
    } else {
        let model_items: Vec<ListItem> = app
            .available_models
            .iter()
            .map(|model| ListItem::new(model.as_str()))
            .collect();

        let models_list = List::new(model_items)
            .highlight_style(Style::default()
                .bg(app.config.theme.parse_color(&app.config.theme.highlight_bg_color))
                .fg(app.config.theme.parse_color(&app.config.theme.highlight_color)))
            .highlight_symbol(">> ");

        f.render_stateful_widget(models_list, inner_area, &mut app.model_list_state);
    }
}

/// Renders a popup for reviewing and approving agent commands.
///
/// Displays detailed information about the currently selected command, including its risk level, description, and command text. Also shows a list of all pending commands with their statuses and highlights the command under review. The popup allows users to approve or reject commands using keyboard input.
///
/// # Examples
///
/// ```
/// // This function is intended to be called within the main UI rendering loop:
/// render_agent_commands_popup(&mut frame, &mut app_state);
/// ```
fn render_agent_commands_popup(f: &mut Frame, app: &mut AppState) {
    let popup_area = centered_rect(80, 70, f.area());
    let block = Block::default()
        .title("Agent Commands - Review and Approve (Y/N to approve/reject)")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    f.render_widget(Clear, popup_area);
    f.render_widget(block, popup_area);

    let inner_area = popup_area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });

    if let Some(current_index) = app.command_approval_index {
        if let Some(cmd) = app.pending_commands.get(current_index) {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(5),
                    Constraint::Min(0),
                ].as_ref())
                .split(inner_area);

            // Command info
            let risk_color = match cmd.risk_level {
                models::RiskLevel::Safe => Color::Green,
                models::RiskLevel::Moderate => Color::Yellow,
                models::RiskLevel::High => Color::Red,
                models::RiskLevel::Critical => Color::Magenta,
            };

            let info_text = format!(
                "Command {}/{} | Risk: {:?}\nDescription: {}",
                current_index + 1,
                app.pending_commands.len(),
                cmd.risk_level,
                cmd.description
            );
            let info_paragraph = Paragraph::new(info_text)
                .style(Style::default().fg(risk_color))
                .wrap(Wrap { trim: false });
            f.render_widget(info_paragraph, chunks[0]);

            // Command text
            let command_paragraph = Paragraph::new(cmd.command.as_str())
                .block(Block::default().borders(Borders::ALL).title("Command"))
                .style(Style::default().fg(Color::Cyan))
                .wrap(Wrap { trim: false });
            f.render_widget(command_paragraph, chunks[1]);

            // All commands list
            let mut command_items = Vec::new();
            for (i, command) in app.pending_commands.iter().enumerate() {
                let status = if command.executed {
                    if command.error.is_some() {
                        "❌ FAILED"
                    } else {
                        "✅ EXECUTED"
                    }
                } else if command.approved {
                    "⏳ APPROVED"
                } else if i == current_index {
                    "👉 PENDING"
                } else {
                    "⏸️ WAITING"
                };
                
                let item_text = format!("{} | {} | {}", status, command.description, command.command);
                let style = if i == current_index {
                    Style::default().bg(Color::DarkGray)
                } else if command.executed && command.error.is_some() {
                    Style::default().fg(Color::Red)
                } else if command.executed {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default()
                };
                
                command_items.push(ListItem::new(item_text).style(style));
            }

            let commands_list = List::new(command_items)
                .block(Block::default().borders(Borders::ALL).title("All Commands"));
            f.render_widget(commands_list, chunks[2]);
        }
    }
}

/// Calculates a rectangle centered within a given area, occupying the specified percentage of width and height.
///
/// The returned rectangle is centered inside `r` and sized to `percent_x`% of the width and `percent_y`% of the height.
///
/// # Examples
///
/// ```
/// use ratatui::layout::Rect;
/// let area = Rect::new(0, 0, 100, 40);
/// let centered = centered_rect(60, 50, area);
/// assert_eq!(centered.width, 60);
/// assert_eq!(centered.height, 20);
/// ```
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

