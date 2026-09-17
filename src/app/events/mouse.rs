use std::io::Result;

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Margin, Rect};

use crate::{
    app::state::context::{App, FilterMenu, NavAction, ViewMode},
    ui::{
        components::{
            header::{filter_menu_area, unit_header_areas},
            unit_list::{UnitActionLayout, UnitRowAction},
        },
        render::main_layout,
    },
};

impl App {
    pub async fn handle_mouse(&mut self, event: MouseEvent) -> Result<()> {
        match event.kind {
            MouseEventKind::ScrollUp => self.perform_nav(NavAction::Up),
            MouseEventKind::ScrollDown => self.perform_nav(NavAction::Down),
            MouseEventKind::ScrollLeft => self.perform_nav(NavAction::Left),
            MouseEventKind::ScrollRight => self.perform_nav(NavAction::Right),
            MouseEventKind::Down(MouseButton::Left) => {
                self.handle_mouse_click(event.column, event.row).await?;
            }
            _ => {}
        }

        Ok(())
    }

    async fn handle_mouse_click(&mut self, column: u16, row: u16) -> Result<()> {
        if self.error_message.is_some() || self.embedded_auth.is_some() {
            return Ok(());
        }

        let [header_area, content_area, _help_area] = main_layout(Rect {
            x: 0,
            y: 0,
            width: self.terminal_size.0,
            height: self.terminal_size.1,
        });

        if self.pending_edit_review.is_some() {
            return Ok(());
        }

        let headers = unit_header_areas(header_area);
        if area_contains(headers[0], column, row) {
            self.unit_list.open_filter_menu = None;
            self.start_search();
            return Ok(());
        }

        self.search.blur();

        if self.unit_list.open_filter_menu.is_some() {
            self.handle_filter_menu_click(column, row, content_area);
            return Ok(());
        }

        if self.view_mode == ViewMode::UnitList && row < header_area.bottom() {
            if let Some(menu) = [
                (headers[1], FilterMenu::Type),
                (headers[2], FilterMenu::Scope),
                (headers[3], FilterMenu::Active),
                (headers[4], FilterMenu::Enablement),
                (headers[5], FilterMenu::Load),
            ]
            .into_iter()
            .find_map(|(area, menu)| area_contains(area, column, row).then_some(menu))
            {
                self.unit_list.open_filter_menu = Some(menu);
            }
            return Ok(());
        }

        let inner = content_area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        });
        if !area_contains(inner, column, row) {
            return Ok(());
        }

        let content_row = row.saturating_sub(content_area.y + 1) as usize;
        match self.view_mode {
            ViewMode::UnitList => {
                self.handle_unit_list_click(column, content_area, content_row)
                    .await?;
            }
            ViewMode::LogView => {
                let index = self.log_view.state.offset() + content_row;
                if index < self.log_view.logs.len()
                    && content_row < content_area.height.saturating_sub(2) as usize
                {
                    self.log_view.state.select(Some(index));
                }
            }
            ViewMode::FileView => {}
        }

        Ok(())
    }

    async fn handle_unit_list_click(
        &mut self,
        column: u16,
        content_area: Rect,
        content_row: usize,
    ) -> Result<()> {
        if content_row >= content_area.height.saturating_sub(2) as usize {
            return Ok(());
        }

        let selected = self.unit_list.state.selected();
        let mut row_start = 0;
        for visible_index in self.unit_list.state.offset()..self.unit_list.filtered_indices.len() {
            let unit_index = self.unit_list.filtered_indices[visible_index];
            let actions = (selected == Some(visible_index)).then(|| {
                UnitActionLayout::new(
                    &self.unit_list.units[unit_index],
                    content_area.width.saturating_sub(2) as usize,
                )
            });
            let row_height = actions
                .as_ref()
                .map_or(1, |layout| 2 + layout.height())
                .min(content_area.height.saturating_sub(2) as usize);
            // Ratatui renders only complete items that fit in the viewport.
            if row_start + row_height > content_area.height.saturating_sub(2) as usize {
                break;
            }
            if content_row < row_start + row_height {
                if let Some(actions) = actions
                    && content_row >= row_start + 2
                {
                    let content_column = column.saturating_sub(content_area.x + 1);
                    let action =
                        actions.action_at(content_column as usize, content_row - row_start - 2);
                    if let Some(action) = action {
                        match action {
                            UnitRowAction::ViewLogs => self.enter_log_view().await,
                            UnitRowAction::ViewFile => self.enter_file_view().await,
                            UnitRowAction::UnitCommand(action) => {
                                self.trigger_selected_unit_command(action).await?
                            }
                        }
                        return Ok(());
                    }
                }

                self.unit_list.select_index(Some(visible_index));
                return Ok(());
            }
            row_start += row_height;
        }

        Ok(())
    }

    fn handle_filter_menu_click(&mut self, column: u16, row: u16, list_area: Rect) {
        let Some(menu) = self.unit_list.open_filter_menu else {
            return;
        };

        let headers = unit_header_areas(
            main_layout(Rect {
                x: 0,
                y: 0,
                width: self.terminal_size.0,
                height: self.terminal_size.1,
            })[0],
        );
        let anchor = match menu {
            FilterMenu::Type => headers[1],
            FilterMenu::Scope => headers[2],
            FilterMenu::Active => headers[3],
            FilterMenu::Enablement => headers[4],
            FilterMenu::Load => headers[5],
        };
        let options = self.filter_menu_options(menu);
        let area = filter_menu_area(
            Rect {
                x: 0,
                y: 0,
                width: self.terminal_size.0,
                height: self.terminal_size.1,
            },
            anchor,
            list_area,
            &options,
        );

        if !area_contains(area, column, row) {
            self.unit_list.open_filter_menu = None;
            return;
        }

        let option_index = row.saturating_sub(area.y + 1) as usize;
        if let Some(option) = options.get(option_index) {
            menu.set_selected_value(self, option.value.clone());
            self.unit_list.open_filter_menu = None;
            self.update_filter(true);
        }
    }
}

fn area_contains(area: Rect, column: u16, row: u16) -> bool {
    column >= area.x && column < area.right() && row >= area.y && row < area.bottom()
}

#[cfg(test)]
mod tests {
    use crossterm::event::KeyModifiers;
    use tokio::sync::mpsc;

    use super::*;

    #[tokio::test]
    async fn clicking_search_header_focuses_search_in_every_view() {
        for view_mode in [ViewMode::UnitList, ViewMode::LogView, ViewMode::FileView] {
            let (tx, _rx) = mpsc::channel(1);
            let mut app = App::blank(tx);
            app.terminal_size = (120, 30);
            app.view_mode = view_mode;
            app.unit_list.open_filter_menu = Some(FilterMenu::Type);

            app.handle_mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 1,
                row: 1,
                modifiers: KeyModifiers::NONE,
            })
            .await
            .unwrap();

            assert!(app.search.is_active, "{view_mode:?}");
            assert_eq!(app.unit_list.open_filter_menu, None, "{view_mode:?}");
        }
    }

    #[tokio::test]
    async fn clicking_away_from_search_blurs_without_clearing_the_query() {
        let (tx, _rx) = mpsc::channel(1);
        let mut app = App::blank(tx);
        app.terminal_size = (120, 30);
        app.search.query = "ssh".to_string();
        app.start_search();

        app.handle_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 1,
            row: 5,
            modifiers: KeyModifiers::NONE,
        })
        .await
        .unwrap();

        assert!(!app.search.is_active);
        assert_eq!(app.search.query, "ssh");
    }
}
