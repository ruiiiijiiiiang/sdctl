use ansi_to_tui::IntoText;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{
    app::state::context::App,
    ui::{
        render::render_scrollbar,
        utils::{search_match_style, selection_style},
    },
};

pub fn draw_log_view(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title(format!(
        " Journal Logs: {} ",
        app.unit_list.selected_key.name
    ));

    if app.is_loading && app.log_view.logs.is_empty() {
        frame.render_widget(
            Paragraph::new("Fetching logs...").centered().block(block),
            area,
        );
    } else if app.log_view.logs.is_empty() {
        frame.render_widget(
            Paragraph::new("No logs found or unauthorized.")
                .centered()
                .block(block),
            area,
        );
    } else {
        let line_range = app.selected_log_line_range();
        let search_query = app.search.query.clone();
        let content_width = area.width.saturating_sub(2);
        let marker_width: u16 = if app.log_view.line_block_select || app.log_view.line_select {
            2
        } else {
            0
        };
        let text_width = content_width.saturating_sub(marker_width);

        let max_rendered_width = app
            .log_view
            .logs
            .iter()
            .map(|line| {
                (match line.as_bytes().into_text() {
                    Ok(t) => t.lines.first().map(|l| l.width()).unwrap_or(0),
                    Err(_) => UnicodeWidthStr::width(line.as_str()),
                }) as u16
            })
            .max()
            .unwrap_or(0);
        let effective_visible = text_width.saturating_sub(3);
        app.log_view.scroll_x = app
            .log_view
            .scroll_x
            .min(max_rendered_width.saturating_sub(effective_visible));

        let selected = app.log_view.state.selected();

        let items: Vec<ListItem> = app
            .log_view
            .logs
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let marker = if app.log_view.line_block_select {
                    match line_range {
                        Some((start, end)) if start == end && i == start => {
                            Span::styled("━ ", Style::default().fg(Color::Green))
                        }
                        Some((start, _)) if i == start => {
                            Span::styled("┏ ", Style::default().fg(Color::Green))
                        }
                        Some((_, end)) if i == end => {
                            Span::styled("┗ ", Style::default().fg(Color::Green))
                        }
                        Some((start, end)) if i >= start && i <= end => {
                            Span::styled("┃ ", Style::default().fg(Color::Green))
                        }
                        _ => Span::raw("┋ "),
                    }
                } else if app.log_view.line_select {
                    if app.log_view.selected_lines.contains(&i) {
                        Span::styled("☑ ", Style::default().fg(Color::Green))
                    } else {
                        Span::raw("☐ ")
                    }
                } else {
                    Span::raw("")
                };
                let should_bold = if app.log_view.line_block_select {
                    line_range
                        .map(|(start, end)| i >= start && i <= end)
                        .unwrap_or(false)
                } else {
                    app.log_view.selected_lines.contains(&i)
                };

                match line.as_bytes().into_text() {
                    Ok(t) => {
                        let mut l = t.lines.into_iter().next().unwrap_or_else(|| Line::from(""));
                        if !search_query.is_empty() && line.contains(&search_query) {
                            l = highlight_exact_match(l, &search_query);
                        }
                        if should_bold {
                            apply_selected_style(&mut l);
                        }
                        let scroll = if Some(i) == selected {
                            app.log_view.scroll_x
                        } else {
                            0
                        };
                        clip_line(&mut l, scroll, text_width);
                        l.spans.insert(0, marker);
                        ListItem::new(l)
                    }
                    Err(_) => {
                        let mut l = Line::from(line.as_str());
                        if !search_query.is_empty() && line.contains(&search_query) {
                            l = highlight_exact_match(l, &search_query);
                        }
                        if should_bold {
                            apply_selected_style(&mut l);
                        }
                        let scroll = if Some(i) == selected {
                            app.log_view.scroll_x
                        } else {
                            0
                        };
                        clip_line(&mut l, scroll, text_width);
                        l.spans.insert(0, marker);
                        ListItem::new(l)
                    }
                }
            })
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(selection_style());

        frame.render_stateful_widget(list, area, &mut app.log_view.state);

        render_scrollbar(
            frame,
            area,
            app.log_view.state.selected().unwrap_or(0),
            app.log_view.logs.len(),
        );
    }
}

fn clip_line(line: &mut Line, scroll_x: u16, max_width: u16) {
    let total_width = line.width() as u16;
    if scroll_x == 0 && total_width <= max_width {
        return;
    }

    let left_reserve: u16 = if scroll_x > 0 { 3 } else { 0 };
    let right_edge = scroll_x + max_width;
    let needs_right_trunc = total_width > right_edge;
    let right_reserve: u16 = if needs_right_trunc { 3 } else { 0 };
    let content_end = scroll_x + max_width.saturating_sub(left_reserve + right_reserve);

    let mut new_spans = Vec::new();
    let mut pos: u16 = 0;

    for span in std::mem::take(&mut line.spans) {
        let content = span.content;
        let style = span.style;
        let span_str = content.as_ref();
        let span_start = pos;

        let mut span_width: u16 = 0;
        let mut char_buf: Option<String> = None;
        for c in span_str.chars() {
            let cw = UnicodeWidthChar::width(c).unwrap_or(0) as u16;
            let char_global = span_start + span_width;

            if char_global < scroll_x {
                span_width += cw;
                continue;
            }

            if char_global + cw > content_end {
                span_width += cw;
                break;
            }

            if char_buf.is_none() {
                let cap = content_end.saturating_sub(char_global) as usize;
                char_buf = Some(String::with_capacity(cap));
            }
            span_width += cw;
            char_buf.as_mut().unwrap().push(c);
        }

        if let Some(clipped) = char_buf {
            new_spans.push(Span::styled(clipped, style));
        }

        pos = span_start + span_width;
        if pos >= content_end {
            break;
        }
    }

    if scroll_x > 0 {
        new_spans.insert(0, Span::styled("...", Style::default().fg(Color::Gray)));
    }
    if needs_right_trunc {
        new_spans.push(Span::styled("...", Style::default().fg(Color::Gray)));
    }
    line.spans = new_spans;
}

fn highlight_exact_match(line: Line<'_>, query: &str) -> Line<'static> {
    let mut spans = Vec::new();
    for span in line.spans {
        let content = span.content;
        let style = span.style;
        if query.is_empty() || !content.contains(query) {
            spans.push(Span::styled(content.to_string(), style));
            continue;
        }

        let mut remaining = content.as_ref();
        while let Some(pos) = remaining.find(query) {
            let (before, after) = remaining.split_at(pos);
            if !before.is_empty() {
                spans.push(Span::styled(before.to_string(), style));
            }
            spans.push(Span::styled(
                query.to_string(),
                style.patch(search_match_style()),
            ));
            remaining = &after[query.len()..];
        }
        if !remaining.is_empty() {
            spans.push(Span::styled(remaining.to_string(), style));
        }
    }

    Line::from(spans)
}

fn apply_selected_style(line: &mut Line<'_>) {
    let bold = Style::default().bold().italic();
    line.style = line.style.patch(bold);
    for span in &mut line.spans {
        span.style = span.style.patch(bold);
    }
}
