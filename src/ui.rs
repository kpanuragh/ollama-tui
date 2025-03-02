use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin},
    style::{Color, Style},
    widgets::{
        Block, BorderType, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState,
    },
    Frame,
};

use crate::app::{App, Mode};

/// Renders the terminal user interface.
pub fn render(app: &mut App, frame: &mut Frame) {
    // Divide the screen into three sections:
    // 1. Chat messages area
    // 2. Input field
    // 3. Model selection
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Min(6),    // Chat messages take most space
                Constraint::Length(3), // Input field
                Constraint::Length(5), // Model selection area
            ]
            .as_ref(),
        )
        .split(frame.area());

    // 🟢 Chat Messages Section
    let messages = app.messages.join("\n");
    let mut title = "Control Mode";
    if Mode::Chat == app.mode {
        title = "Chat Mode";
    } else if Mode::Model == app.mode {
        title = "Model Mode";
    }
    let chat_area = Paragraph::new(messages)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .title_alignment(Alignment::Center)
                .border_type(BorderType::Rounded),
        )
        .scroll((app.message_scroll as u16, 0))
        .style(Style::default().fg(Color::White).bg(Color::Black));
    frame.render_widget(chat_area, chunks[0]);

    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓"));
    let mut scrollbar_state = ScrollbarState::new(app.messages.len()).position(app.message_scroll);
    frame.render_stateful_widget(
        scrollbar,
        frame.area().inner(Margin {
            // using an inner vertical margin of 1 unit makes the scrollbar inside the block
            vertical: 1,
            horizontal: 0,
        }),
        &mut scrollbar_state,
    );

    // 🔵 Input Field Section
    let input_area = Paragraph::new(format!("> {}", app.input))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Input")
                .title_alignment(Alignment::Center)
                .border_type(BorderType::Rounded),
        )
        .style(Style::default().fg(Color::Yellow).bg(Color::Black));

    frame.render_widget(input_area, chunks[1]);

    // 🟠 Model Selection Section
    let model_list: Vec<ListItem> = app
        .models
        .iter()
        .skip(app.model_scroll)
        .map(|m| {
            let is_selected = *m == app.model;
            let style = if is_selected {
                Style::default().fg(Color::Black).bg(Color::Green)
            } else {
                Style::default().fg(Color::White).bg(Color::Black)
            };
            ListItem::new(m.clone()).style(style)
        })
        .collect();

    let model_widget = List::new(model_list)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Models (Use ↑/↓ to Select)")
                .title_alignment(Alignment::Center)
                .border_type(BorderType::Rounded),
        )
        .style(Style::default().fg(Color::Green).bg(Color::Black));

    frame.render_widget(model_widget, chunks[2]);
}
