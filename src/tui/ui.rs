use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use super::app::App;

/// Draw the TUI layout.
pub fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // search input
            Constraint::Min(5),    // results
            Constraint::Length(3), // status + help
        ])
        .split(frame.area());

    draw_search_input(frame, app, chunks[0]);
    draw_results(frame, app, chunks[1]);
    draw_status_bar(frame, app, chunks[2]);

    // Set cursor position when input is focused
    if app.input_focused {
        frame.set_cursor_position((chunks[0].x + 1 + app.cursor_pos as u16, chunks[0].y + 1));
    }
}

/// Draw the search input box.
fn draw_search_input(frame: &mut Frame, app: &App, area: Rect) {
    let style = if app.input_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let input = Paragraph::new(app.query.as_str())
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(style)
                .title(" Search PRs (semantic) "),
        );

    frame.render_widget(input, area);
}

/// Draw the results list.
fn draw_results(frame: &mut Frame, app: &App, area: Rect) {
    if app.results.is_empty() {
        let empty_msg = if app.query.is_empty() {
            "Type a query and press Enter to search"
        } else {
            "No results"
        };

        let paragraph = Paragraph::new(empty_msg)
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL).title(" Results "))
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);
        return;
    }

    let items: Vec<ListItem> = app
        .results
        .iter()
        .enumerate()
        .map(|(i, result)| {
            let is_selected = i == app.selected;

            let score_color = if result.score > 0.7 {
                Color::Green
            } else if result.score > 0.4 {
                Color::Yellow
            } else {
                Color::Red
            };

            let state_color = match result.state.as_str() {
                "open" => Color::Green,
                "merged" => Color::Magenta,
                "closed" => Color::Red,
                _ => Color::Gray,
            };

            let marker = if is_selected { ">" } else { " " };

            let labels_str = if result.labels.is_empty() {
                String::new()
            } else {
                format!(" [{}]", result.labels.join(", "))
            };

            let line = Line::from(vec![
                Span::styled(
                    format!("{marker} "),
                    Style::default().fg(if is_selected {
                        Color::Cyan
                    } else {
                        Color::DarkGray
                    }),
                ),
                Span::styled(
                    format!("{:.0}% ", result.score * 100.0),
                    Style::default().fg(score_color),
                ),
                Span::styled(
                    format!("#{:<5} ", result.number),
                    Style::default().fg(Color::Blue),
                ),
                Span::styled(
                    format!("[{}] ", result.state),
                    Style::default().fg(state_color),
                ),
                Span::styled(
                    result.title.clone(),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(if is_selected {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ),
                Span::styled(labels_str, Style::default().fg(Color::Yellow)),
                Span::styled(
                    format!("  @{}", result.author),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);

            ListItem::new(line)
        })
        .collect();

    let border_style = if !app.input_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(format!(" Results ({}) ", app.results.len())),
    );

    frame.render_widget(list, area);
}

/// Draw the status bar and keyboard help.
fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(2)])
        .split(area);

    // Status message
    let status = Paragraph::new(app.status.as_str()).style(Style::default().fg(Color::Cyan));
    frame.render_widget(status, chunks[0]);

    // Keyboard help
    let help_text = if app.input_focused {
        "Enter: search | Tab: results | Esc: quit"
    } else {
        "j/k or Up/Down: navigate | Enter/o: open PR | Tab: search | /: search | Esc: quit"
    };

    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::TOP));

    frame.render_widget(help, chunks[1]);
}
