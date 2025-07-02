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

pub fn ui(f: &mut Frame, app: &mut AppState) {
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(75), Constraint::Percentage(25)].as_ref())
        .split(f.size());

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3), Constraint::Length(1)].as_ref())
        .split(main_chunks[0]);

    let chat_border_style = if app.mode == AppMode::Normal {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let sessions_border_style = if app.mode == AppMode::SessionSelection {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let chat_messages = render_messages(app.current_messages(), left_chunks[0].width);
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
    } else {
        app.input.clone()
    };
    let input_paragraph = Paragraph::new(input_text.as_str())
        .block(Block::default().borders(Borders::ALL).title("Input"));
    f.render_widget(input_paragraph, left_chunks[1]);

    let status_bar_text = format!(
        "Model: {} | Ctrl+D: Clear | Ctrl+L: Models | Tab: Sessions | Ctrl+C: Quit",
        app.current_model
    );
    let status_bar = Paragraph::new(status_bar_text).style(Style::default().fg(Color::DarkGray));
    f.render_widget(status_bar, left_chunks[2]);

    if !app.is_loading && app.mode == AppMode::Normal {
        f.set_cursor(
            left_chunks[1].x + app.input.width() as u16 + 1,
            left_chunks[1].y + 1,
        );
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
        .highlight_style(Style::default().bg(Color::LightGreen).fg(Color::Black))
        .highlight_symbol(">> ");

    f.render_stateful_widget(sessions_list, main_chunks[1], &mut app.session_list_state);

    if app.mode == AppMode::ModelSelection {
        render_model_selection_popup(f, app);
    }
}

fn render_model_selection_popup(f: &mut Frame, app: &mut AppState) {
    let popup_area = centered_rect(60, 50, f.size());
    let block = Block::default()
        .title("Select a Model (Enter to confirm, Esc/q to cancel)")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

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
                .bg(Color::LightGreen)
                .fg(Color::Black)
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

fn render_messages<'a>(messages: &'a [models::Message], width: u16) -> Text<'a> {
    let mut lines = Vec::new();
    for message in messages {
        let style = match message.role {
            models::Role::User => Style::default().fg(Color::Cyan),
            models::Role::Assistant => Style::default().fg(Color::LightGreen),
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

