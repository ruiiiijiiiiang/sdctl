use std::{collections::VecDeque, ops::Range};

use ratatui::{
    Frame,
    layout::{Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use unicode_width::UnicodeWidthChar;

use crate::{
    app::state::context::App,
    models::{UnitAction, UnitInfo, UnitType},
    ui::{
        render::render_scrollbar,
        utils::{UNIT_COLUMN_CONSTRAINTS, keybind_style, selection_style},
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnitRowAction {
    ViewLogs,
    ViewFile,
    UnitCommand(UnitAction),
}

#[derive(Clone, Copy)]
struct ActionGroup {
    key: &'static str,
    helper: &'static str,
    action: UnitRowAction,
}

const VIEW_LOGS_ACTION: ActionGroup = ActionGroup {
    key: "l/Enter",
    helper: "view log",
    action: UnitRowAction::ViewLogs,
};

const VIEW_FILE_ACTION: ActionGroup = ActionGroup {
    key: "f",
    helper: "view unit file",
    action: UnitRowAction::ViewFile,
};

const SERVICE_ACTIONS: [ActionGroup; 3] = [
    ActionGroup {
        key: "s",
        helper: "start",
        action: UnitRowAction::UnitCommand(UnitAction::Start),
    },
    ActionGroup {
        key: "t",
        helper: "stop",
        action: UnitRowAction::UnitCommand(UnitAction::Stop),
    },
    ActionGroup {
        key: "r",
        helper: "restart",
        action: UnitRowAction::UnitCommand(UnitAction::Restart),
    },
];

const ENABLEMENT_ACTIONS: [ActionGroup; 4] = [
    ActionGroup {
        key: "e",
        helper: "enable",
        action: UnitRowAction::UnitCommand(UnitAction::Enable),
    },
    ActionGroup {
        key: "d",
        helper: "disable",
        action: UnitRowAction::UnitCommand(UnitAction::Disable),
    },
    ActionGroup {
        key: "m",
        helper: "mask",
        action: UnitRowAction::UnitCommand(UnitAction::Mask),
    },
    ActionGroup {
        key: "u",
        helper: "unmask",
        action: UnitRowAction::UnitCommand(UnitAction::Unmask),
    },
];

const MAINTENANCE_ACTIONS: [ActionGroup; 2] = [
    ActionGroup {
        key: "R",
        helper: "reload",
        action: UnitRowAction::UnitCommand(UnitAction::Reload),
    },
    ActionGroup {
        key: "x",
        helper: "reset-failed",
        action: UnitRowAction::UnitCommand(UnitAction::ResetFailed),
    },
];

pub fn draw_unit_list(frame: &mut Frame, app: &mut App, area: Rect) {
    let content_width = area.width.saturating_sub(2) as usize;
    let list_block = Block::default().borders(Borders::ALL).title(format!(
        " Units ({}/{}) ",
        app.unit_list.filtered_indices.len(),
        app.unit_list.units.len()
    ));

    if app.is_loading && app.unit_list.units.is_empty() {
        frame.render_widget(
            Paragraph::new("Loading units...")
                .centered()
                .block(list_block),
            area,
        );
    } else {
        let column_widths = unit_row_column_widths(area.width.saturating_sub(2));
        let selected_index = app.selected_unit_index();
        let items: Vec<ListItem> = app
            .unit_list
            .filtered_indices
            .iter()
            .enumerate()
            .map(|(visible_index, &i)| {
                let unit = &app.unit_list.units[i];
                let mut lines = format_unit_row(
                    unit,
                    &column_widths,
                    content_width,
                    selected_index == Some(visible_index),
                );
                // A List item taller than its viewport cannot be displayed by ratatui.
                lines.truncate(area.height.saturating_sub(2) as usize);
                ListItem::new(lines)
            })
            .collect();

        let list = List::new(items)
            .block(list_block)
            .highlight_style(selection_style().bold());

        frame.render_stateful_widget(list, area, &mut app.unit_list.state);

        render_scrollbar(
            frame,
            area,
            selected_index.unwrap_or(0),
            app.unit_list.filtered_indices.len(),
        );
    }
}

fn format_unit_row(
    unit: &UnitInfo,
    widths: &[usize; 6],
    content_width: usize,
    is_selected: bool,
) -> Vec<Line<'static>> {
    let active_sub = format!("{} ({})", unit.active_state, unit.sub_state);
    let unit_type = UnitType::from_unit_name(&unit.name);
    let mut lines = vec![Line::from(vec![
        Span::styled(
            format_cell(&unit.name, widths[0], CellAlign::Left),
            Style::default().bold(),
        ),
        Span::styled(
            format_cell(unit_type.as_ref(), widths[1], CellAlign::Center),
            Style::default().fg(unit_type.color()),
        ),
        Span::styled(
            format_cell(unit.scope.as_ref(), widths[2], CellAlign::Center),
            Style::default().fg(unit.scope.color()),
        ),
        Span::styled(
            format_cell(&active_sub, widths[3], CellAlign::Center),
            Style::default().fg(unit.active_state.color()),
        ),
        Span::styled(
            format_cell(unit.enablement_state.as_ref(), widths[4], CellAlign::Center),
            Style::default().fg(unit.enablement_state.color()),
        ),
        Span::styled(
            format_cell(unit.load_state.as_ref(), widths[5], CellAlign::Center),
            Style::default().fg(unit.load_state.color()),
        ),
    ])];

    if is_selected {
        let detail = Line::from(vec![
            Span::raw(" ┣  "),
            Span::styled("Description: ", Style::default().bold()),
            Span::styled(unit.description.clone(), Style::default().fg(Color::White)),
            Span::styled("   Unit file path: ", Style::default().bold()),
            Span::styled(
                if unit.fragment_path.is_empty() {
                    "N/A".to_string()
                } else {
                    unit.fragment_path.clone()
                },
                Style::default().fg(Color::White),
            ),
        ]);
        lines.push(clip_line(detail, content_width));

        lines.extend(UnitActionLayout::new(unit, content_width).lines);
    }

    lines
}

pub fn unit_row_column_widths(total_width: u16) -> [usize; 6] {
    let columns = Layout::horizontal([
        UNIT_COLUMN_CONSTRAINTS[0],
        UNIT_COLUMN_CONSTRAINTS[1],
        UNIT_COLUMN_CONSTRAINTS[2],
        UNIT_COLUMN_CONSTRAINTS[3],
        UNIT_COLUMN_CONSTRAINTS[4],
        UNIT_COLUMN_CONSTRAINTS[5],
    ])
    .split(Rect {
        x: 0,
        y: 0,
        width: total_width,
        height: 1,
    });

    [
        columns[0].width as usize,
        columns[1].width as usize,
        columns[2].width as usize,
        columns[3].width as usize,
        columns[4].width as usize,
        columns[5].width as usize,
    ]
}

#[derive(Debug)]
struct ActionHit {
    row: usize,
    columns: Range<usize>,
    action: UnitRowAction,
}

/// Text and clickable ranges are generated together in terminal cell coordinates.
pub struct UnitActionLayout {
    lines: Vec<Line<'static>>,
    hits: Vec<ActionHit>,
}

impl UnitActionLayout {
    pub fn new(unit: &UnitInfo, width: usize) -> Self {
        let mut layout = Self {
            lines: Vec::new(),
            hits: Vec::new(),
        };
        if width == 0 {
            return layout;
        }
        let widths = unit_row_column_widths(width.min(u16::MAX as usize) as u16);
        let primary_groups =
            filter_applicable_action_groups(unit, [VIEW_LOGS_ACTION, VIEW_FILE_ACTION]);
        let full_prefix = Line::from(vec![
            Span::raw(" ┗  "),
            Span::styled("Action: ", Style::default().bold()),
        ]);
        let minimum_action_width = primary_groups
            .iter()
            .map(|group| group.key.chars().count() + 1)
            .max()
            .unwrap_or(0);
        let prefix = if widths[0] >= full_prefix.width() + minimum_action_width {
            full_prefix
        } else {
            Line::default()
        };
        let prefix_width = prefix.width();
        let primary_width = widths[0].saturating_sub(prefix_width);

        let mut primary = wrap_action_groups(primary_groups, primary_width, primary_width);
        if primary.is_empty() && widths[0] > 0 {
            primary.push(ActionCellRow::default());
        }
        let service = wrap_action_groups(
            filter_applicable_action_groups(unit, SERVICE_ACTIONS),
            widths[3],
            widths[3],
        );
        let enablement = wrap_action_groups(
            filter_applicable_action_groups(unit, ENABLEMENT_ACTIONS),
            widths[4],
            widths[4],
        );
        let maintenance = wrap_action_groups(
            filter_applicable_action_groups(unit, MAINTENANCE_ACTIONS),
            widths[5],
            widths[5],
        );
        let height = [
            primary.len(),
            service.len(),
            enablement.len(),
            maintenance.len(),
        ]
        .into_iter()
        .max()
        .unwrap_or(0);

        for row in 0..height {
            let mut spans = Vec::new();
            let mut column = 0;
            for (column_index, column_width) in widths.into_iter().enumerate() {
                let cell_row = match column_index {
                    0 => primary.get(row),
                    3 => service.get(row),
                    4 => enablement.get(row),
                    5 => maintenance.get(row),
                    _ => None,
                };
                if column_index == 0 {
                    if row == 0 {
                        spans.extend(prefix.spans.clone());
                    } else {
                        spans.push(Span::raw(" ".repeat(prefix_width)));
                    }
                    spans.extend(layout.render_action_cell(
                        cell_row,
                        primary_width,
                        column + prefix_width,
                        row,
                        CellAlign::Left,
                    ));
                } else {
                    spans.extend(layout.render_action_cell(
                        cell_row,
                        column_width,
                        column,
                        row,
                        CellAlign::Center,
                    ));
                }
                column += column_width;
            }
            layout.lines.push(Line::from(spans));
        }
        layout
    }

    fn render_action_cell(
        &mut self,
        cell: Option<&ActionCellRow>,
        width: usize,
        column: usize,
        row: usize,
        align: CellAlign,
    ) -> Vec<Span<'static>> {
        let Some(cell) = cell else {
            return vec![Span::raw(" ".repeat(width))];
        };
        let content_width = cell.width();
        let left = match align {
            CellAlign::Left => 0,
            CellAlign::Center => width.saturating_sub(content_width) / 2,
        };
        let right = width.saturating_sub(content_width + left);
        let mut spans = vec![Span::raw(" ".repeat(left))];
        let mut action_column = column + left;
        for (index, piece) in cell.pieces.iter().enumerate() {
            if index > 0 {
                spans.push(Span::raw("  "));
                action_column += 2;
            }
            let piece_width = piece.text.width();
            self.hits.push(ActionHit {
                row,
                columns: action_column..action_column + piece_width,
                action: piece.action,
            });
            spans.extend(piece.text.spans.clone());
            action_column += piece_width;
        }
        spans.push(Span::raw(" ".repeat(right)));
        spans
    }

    pub fn height(&self) -> usize {
        self.lines.len()
    }

    pub fn action_at(&self, column: usize, row: usize) -> Option<UnitRowAction> {
        self.hits
            .iter()
            .find(|hit| hit.row == row && hit.columns.contains(&column))
            .map(|hit| hit.action)
    }
}

#[derive(Default)]
struct ActionCellRow {
    pieces: Vec<ActionPiece>,
}

impl ActionCellRow {
    fn width(&self) -> usize {
        self.pieces
            .iter()
            .map(|piece| piece.text.width())
            .sum::<usize>()
            + self.pieces.len().saturating_sub(1) * 2
    }
}

struct ActionPiece {
    action: UnitRowAction,
    text: Line<'static>,
}

fn wrap_action_groups(
    groups: Vec<ActionGroup>,
    first_width: usize,
    following_width: usize,
) -> Vec<ActionCellRow> {
    if groups.is_empty() || following_width == 0 {
        return Vec::new();
    }

    let mut pending: VecDeque<ActionPiece> = groups
        .into_iter()
        .map(|group| ActionPiece {
            action: group.action,
            text: Line::from(vec![
                Span::styled(group.key, keybind_style()),
                Span::raw(format!(" {}", group.helper)),
            ]),
        })
        .collect();
    let mut rows = Vec::new();

    while !pending.is_empty() {
        let row_width = if rows.is_empty() {
            first_width
        } else {
            following_width
        };
        if row_width == 0 {
            rows.push(ActionCellRow::default());
            continue;
        }

        let mut row = ActionCellRow::default();
        loop {
            let Some(piece) = pending.front() else {
                break;
            };
            let separator_width = usize::from(!row.pieces.is_empty()) * 2;
            let remaining = row_width.saturating_sub(row.width());
            if separator_width + piece.text.width() <= remaining {
                row.pieces.push(pending.pop_front().unwrap());
                continue;
            }

            if !row.pieces.is_empty() {
                break;
            }
            // Do not split the first primary action merely to place it after the label.
            if rows.is_empty() && first_width < following_width {
                break;
            }

            let piece = pending.pop_front().unwrap();
            let (head, tail) = split_line_at_width(piece.text, row_width);
            row.pieces.push(ActionPiece {
                action: piece.action,
                text: head,
            });
            if let Some(tail) = tail {
                pending.push_front(ActionPiece {
                    action: piece.action,
                    text: tail,
                });
            }
            break;
        }
        rows.push(row);
    }
    rows
}

fn split_line_at_width(
    line: Line<'static>,
    width: usize,
) -> (Line<'static>, Option<Line<'static>>) {
    let mut head = Vec::new();
    let mut tail = Vec::new();
    let mut used = 0;
    for span in line.spans {
        for ch in span.content.chars() {
            let char_width = ch.width().unwrap_or(0);
            let target = if used + char_width <= width {
                used += char_width;
                &mut head
            } else {
                &mut tail
            };
            target.push(Span::styled(ch.to_string(), span.style));
        }
    }

    let tail = (!tail.is_empty()).then(|| Line::from(tail));
    (Line::from(head), tail)
}

fn filter_applicable_action_groups(
    unit: &UnitInfo,
    groups: impl IntoIterator<Item = ActionGroup>,
) -> Vec<ActionGroup> {
    groups
        .into_iter()
        .filter(|group| match group.action {
            UnitRowAction::UnitCommand(action) => action.is_applicable_to(unit),
            UnitRowAction::ViewLogs => true,
            UnitRowAction::ViewFile => !unit.fragment_path.is_empty(),
        })
        .collect()
}

#[derive(Clone, Copy)]
enum CellAlign {
    Left,
    Center,
}

fn format_cell(value: &str, width: usize, align: CellAlign) -> String {
    if width == 0 {
        return String::new();
    }

    let clipped = clip_text(value, width);
    let clipped_width = clipped.chars().count();
    let padding = width.saturating_sub(clipped_width);

    match align {
        CellAlign::Left => format!("{clipped}{:padding$}", "", padding = padding),
        CellAlign::Center => {
            let left = padding / 2;
            let right = padding.saturating_sub(left);
            format!(
                "{:left$}{clipped}{:right$}",
                "",
                "",
                left = left,
                right = right
            )
        }
    }
}

fn clip_text(value: &str, width: usize) -> String {
    let length = value.chars().count();
    if length <= width {
        return value.to_string();
    }

    if width <= 3 {
        return value.chars().take(width).collect();
    }

    let mut clipped: String = value.chars().take(width - 3).collect();
    clipped.push_str("...");
    clipped
}

fn clip_line(line: Line<'static>, width: usize) -> Line<'static> {
    if width == 0 {
        return Line::from(Vec::<Span<'static>>::new());
    }

    let mut spans = Vec::new();
    let mut remaining = width;

    for span in line.spans {
        if remaining == 0 {
            break;
        }

        let text = span.content.to_string();
        let text_width = text.chars().count();
        if text_width <= remaining {
            spans.push(Span::styled(text, span.style));
            remaining -= text_width;
            continue;
        }

        if remaining <= 3 {
            spans.push(Span::styled(
                text.chars().take(remaining).collect::<String>(),
                span.style,
            ));
            break;
        }

        let clipped: String = text.chars().take(remaining - 3).collect();
        spans.push(Span::styled(format!("{}...", clipped), span.style));
        break;
    }

    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, widgets::Widget};
    use tokio::sync::mpsc;
    use zbus::zvariant::OwnedObjectPath;

    use crate::models::{UnitAction, UnitActiveState, UnitInfo, UnitType};

    fn unit() -> UnitInfo {
        UnitInfo {
            name: "ssh.service".to_string(),
            description: "Secure Shell".to_string(),
            scope: crate::models::UnitScope::Global,
            load_state: crate::models::UnitLoadState::Loaded,
            active_state: crate::models::UnitActiveState::Active,
            enablement_state: crate::models::UnitEnablementState::Enabled,
            can_reload: true,
            sub_state: "running".to_string(),
            path: OwnedObjectPath::try_from("/test/unit/ssh").unwrap(),
            fragment_path: "/etc/systemd/system/ssh.service".to_string(),
        }
    }

    #[test]
    fn selected_row_gets_action_line_with_detail_prefix() {
        let lines = format_unit_row(&unit(), &[10, 10, 10, 10, 10, 10], 80, true);

        assert!(lines.len() > 3);
        assert!(lines[0].spans[1].content.contains("service"));
        assert!(lines[1].spans[0].content.starts_with(" ┣ "));
        assert!(
            lines[1]
                .spans
                .iter()
                .any(|span| span.content.contains("Secure Shell"))
        );
        assert!(
            lines[1]
                .spans
                .iter()
                .any(|span| span.content.contains("/etc/systemd/system/ssh.service"))
        );
        assert!(lines[2].spans[0].content.starts_with(" ┗ "));
        assert!(lines[2].spans.iter().any(|span| span.content == "Action: "));
    }

    #[test]
    fn active_unit_hides_start_but_keeps_stop_visible() {
        let lines = format_unit_row(&unit(), &[60, 10, 10, 40, 40, 40], 200, true);
        let action_text: String = lines[2]
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();

        assert!(!action_text.contains("s start"));
        assert!(action_text.contains("t stop"));
    }

    #[test]
    fn action_cells_stay_centered_inside_their_semantic_columns() {
        let mut unit = unit();
        unit.active_state = UnitActiveState::Unknown;
        let width = 120;
        let widths = unit_row_column_widths(width);
        let starts = [
            0,
            widths[0],
            widths[0] + widths[1],
            widths[0] + widths[1] + widths[2],
            widths[0] + widths[1] + widths[2] + widths[3],
            widths[0] + widths[1] + widths[2] + widths[3] + widths[4],
        ];
        let layout = UnitActionLayout::new(&unit, width as usize);

        for hit in &layout.hits {
            let column_index = match hit.action {
                UnitRowAction::ViewLogs | UnitRowAction::ViewFile => 0,
                UnitRowAction::UnitCommand(
                    UnitAction::Start | UnitAction::Stop | UnitAction::Restart,
                ) => 3,
                UnitRowAction::UnitCommand(
                    UnitAction::Enable
                    | UnitAction::Disable
                    | UnitAction::Mask
                    | UnitAction::Unmask,
                ) => 4,
                UnitRowAction::UnitCommand(UnitAction::Reload | UnitAction::ResetFailed) => 5,
            };
            assert!(hit.columns.start >= starts[column_index], "{hit:?}");
            assert!(
                hit.columns.end <= starts[column_index] + widths[column_index],
                "{hit:?}"
            );
        }

        let start_hit = layout
            .hits
            .iter()
            .find(|hit| hit.action == UnitRowAction::UnitCommand(UnitAction::Start))
            .unwrap();
        let active_start = starts[3];
        let active_end = active_start + widths[3];
        assert!(start_hit.columns.start >= active_start);
        assert!(start_hit.columns.end <= active_end);
        assert_eq!(layout.action_at(active_start, start_hit.row), None);
        assert_eq!(layout.action_at(active_end - 1, start_hit.row), None);

        let view_action_start = " ┗  Action: ".chars().count();
        for action in [UnitRowAction::ViewLogs, UnitRowAction::ViewFile] {
            let hit = layout.hits.iter().find(|hit| hit.action == action).unwrap();
            assert_eq!(hit.columns.start, view_action_start);
        }

        for column_index in [3, 4, 5] {
            for row in 0..layout.height() {
                let hits: Vec<_> = layout
                    .hits
                    .iter()
                    .filter(|hit| {
                        hit.row == row
                            && hit.columns.start >= starts[column_index]
                            && hit.columns.end <= starts[column_index] + widths[column_index]
                    })
                    .collect();
                if let (Some(first), Some(last)) = (hits.first(), hits.last()) {
                    let left = first.columns.start - starts[column_index];
                    let right = starts[column_index] + widths[column_index] - last.columns.end;
                    assert!(
                        left.abs_diff(right) <= 1,
                        "column {column_index}, row {row}"
                    );
                }
            }
        }
    }

    #[test]
    fn wrapped_action_text_and_hits_agree_at_every_width() {
        let mut unit = unit();
        unit.active_state = UnitActiveState::Unknown;
        let expected = [
            (UnitRowAction::ViewLogs, "l/Enter view log"),
            (UnitRowAction::ViewFile, "f view unit file"),
            (UnitRowAction::UnitCommand(UnitAction::Start), "s start"),
            (UnitRowAction::UnitCommand(UnitAction::Stop), "t stop"),
            (UnitRowAction::UnitCommand(UnitAction::Restart), "r restart"),
            (UnitRowAction::UnitCommand(UnitAction::Enable), "e enable"),
            (UnitRowAction::UnitCommand(UnitAction::Disable), "d disable"),
            (UnitRowAction::UnitCommand(UnitAction::Mask), "m mask"),
            (UnitRowAction::UnitCommand(UnitAction::Unmask), "u unmask"),
            (UnitRowAction::UnitCommand(UnitAction::Reload), "R reload"),
            (
                UnitRowAction::UnitCommand(UnitAction::ResetFailed),
                "x reset-failed",
            ),
        ];
        assert_eq!(UnitActionLayout::new(&unit, 0).height(), 0);
        for width in 20..=200 {
            let layout = UnitActionLayout::new(&unit, width);
            assert!(layout.lines.iter().all(|line| line.width() == width));
            let area = Rect::new(0, 0, width as u16, layout.height() as u16);
            let mut buffer = Buffer::empty(area);
            Paragraph::new(layout.lines.clone()).render(area, &mut buffer);
            for (action, label) in expected {
                let mut rendered = String::new();
                for row in 0..layout.height() {
                    for column in 0..width {
                        if layout.action_at(column, row) == Some(action) {
                            rendered.push_str(buffer[(column as u16, row as u16)].symbol());
                        }
                    }
                    assert_eq!(layout.action_at(width, row), None);
                }
                assert_eq!(rendered, label, "{action:?} at width {width}");
            }
            assert_eq!(layout.action_at(0, layout.height()), None);
        }
        unit.fragment_path.clear();
        unit.active_state = UnitActiveState::Active;
        unit.can_reload = false;
        let layout = UnitActionLayout::new(&unit, 80);
        for hit in layout.hits {
            assert!(!matches!(
                hit.action,
                UnitRowAction::ViewFile
                    | UnitRowAction::UnitCommand(UnitAction::Start | UnitAction::Reload)
            ));
        }
    }

    #[tokio::test]
    async fn clicking_below_wrapped_actions_selects_the_rendered_unit_after_resize() {
        for width in [50, 80, 160] {
            let (tx, _rx) = mpsc::channel(1);
            let mut app = App::blank(tx);
            app.is_loading = false;
            app.unit_list.units = (0..30)
                .map(|i| {
                    let mut unit = unit();
                    unit.name = format!("unit{i:02}.service");
                    unit
                })
                .collect();
            app.unit_list.filtered_indices = (0..30).collect();
            app.unit_list.select_index(Some(18));
            *app.unit_list.state.offset_mut() = 16;
            let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
            terminal
                .draw(|frame| crate::ui::render::draw(frame, &mut app))
                .unwrap();
            terminal.backend_mut().resize(width, 30);
            terminal.resize(Rect::new(0, 0, width, 30)).unwrap();
            terminal
                .draw(|frame| crate::ui::render::draw(frame, &mut app))
                .unwrap();
            assert!(app.unit_list.state.offset() > 0);
            let content = crate::ui::render::main_layout(Rect::new(0, 0, width, 30))[1];
            let buffer = terminal.backend().buffer();
            let next_row = (content.y + 1..content.bottom() - 1)
                .find(|&y| {
                    let text: String = (content.x + 1..content.right() - 1)
                        .map(|x| buffer[(x, y)].symbol())
                        .collect();
                    text.starts_with("unit19.service")
                })
                .expect("next unit is visible after the expanded actions");
            let click = |column, row| MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column,
                row,
                modifiers: KeyModifiers::NONE,
            };
            // Border clicks must never select a row or trigger an action.
            app.handle_mouse(click(content.x, next_row)).await.unwrap();
            assert_eq!(app.unit_list.state.selected(), Some(18));
            app.handle_mouse(click(content.x + 1, next_row))
                .await
                .unwrap();
            assert_eq!(app.unit_list.state.selected(), Some(19));
            assert_eq!(app.unit_list.selected_key.name, "unit19.service");
        }
    }

    #[test]
    fn tiny_viewports_keep_the_selected_unit_visible() {
        let (tx, _rx) = mpsc::channel(1);
        let mut app = App::blank(tx);
        app.is_loading = false;
        app.unit_list.units = vec![unit()];
        app.unit_list.filtered_indices = vec![0];
        app.unit_list.select_index(Some(0));
        let mut terminal = Terminal::new(TestBackend::new(22, 5)).unwrap();
        terminal
            .draw(|frame| draw_unit_list(frame, &mut app, frame.area()))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let first_cell: String = (1..7).map(|x| buffer[(x, 1)].symbol()).collect();
        assert_eq!(first_cell, "ssh...");
    }

    #[test]
    fn format_unit_row_uses_unknown_type_for_unrecognized_suffix() {
        let mut unknown = unit();
        unknown.name = "ssh.whatever".to_string();

        let lines = format_unit_row(&unknown, &[10, 10, 10, 10, 10, 10], 80, false);

        assert_eq!(lines.len(), 1);
        assert!(
            lines[0]
                .spans
                .iter()
                .any(|span| span.content.contains("unknown"))
        );
        assert_eq!(UnitType::from_unit_name(&unknown.name), UnitType::Unknown);
    }
}
