use std::collections::HashSet;

use ratatui::widgets::ListState;
use tokio::spawn;
use tokio_stream::StreamExt;

use crate::{
    app::{
        state::context::{App, ViewMode},
        utils::LOG_LINE_LIMIT,
    },
    models::AppInternalEvent,
    systemd::journal::JournalManager,
};

#[derive(Default)]
pub struct LogViewState {
    pub logs: Vec<String>,
    pub state: ListState,
    pub line_select: bool,
    pub line_block_select: bool,
    pub selected_lines: HashSet<usize>,
    pub line_marks: Vec<usize>,
    pub scroll_x: u16,
    pub is_following: bool,
    pub follow_task: Option<tokio::task::JoinHandle<()>>,
}

impl LogViewState {
    pub fn move_selection(&mut self, delta: i32) {
        if self.logs.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                let next = i as i32 + delta;
                if next < 0 {
                    0
                } else if next >= self.logs.len() as i32 {
                    self.logs.len() - 1
                } else {
                    next as usize
                }
            }
            None => {
                if delta > 0 {
                    0
                } else {
                    self.logs.len().saturating_sub(1)
                }
            }
        };
        self.state.select(Some(i));
        self.scroll_x = 0;
    }

    pub fn toggle_line_mark(&mut self) {
        let Some(index) = self.state.selected() else {
            return;
        };

        if self.line_marks.contains(&index) {
            self.line_marks.retain(|&i| i != index);
            return;
        }

        if self.line_marks.len() == 2 {
            self.line_marks.remove(0);
        }

        self.line_marks.push(index);
    }

    pub fn selected_line_range(&self) -> Option<(usize, usize)> {
        match self.line_marks.as_slice() {
            [only] => Some((*only, *only)),
            [start, end] => Some(((*start).min(*end), (*start).max(*end))),
            _ => None,
        }
    }

    pub fn selected_lines_text(&self) -> Option<String> {
        let (start, end) = self.selected_line_range()?;
        let lines: Vec<&str> = (start..=end)
            .filter_map(|index| self.logs.get(index).map(|line| line.as_str()))
            .collect();

        if lines.is_empty() {
            None
        } else {
            Some(lines.join("\n"))
        }
    }

    pub fn clear_visual_modes(&mut self) {
        self.line_select = false;
        self.line_block_select = false;
        self.selected_lines.clear();
        self.line_marks.clear();
    }
}

impl App {
    pub async fn enter_log_view(&mut self) {
        if let Some(unit) = self.get_selected_unit() {
            let name = unit.name.clone();
            let scope = unit.scope.to_string();
            self.stop_following_logs();
            self.search.clear();
            self.view_mode = ViewMode::LogView;
            self.log_view.logs.clear();
            self.log_view.state.select(None);
            self.log_view.scroll_x = 0;
            self.log_view.clear_visual_modes();
            self.fetch_unit_logs(name, scope, false).await;
        }
    }

    pub async fn fetch_unit_logs(&mut self, unit_name: String, scope: String, is_manual: bool) {
        self.is_loading = true;
        let tx = self.internal_tx.clone();
        spawn(async move {
            let manager = JournalManager::new();
            match manager.fetch_logs(&unit_name, &scope, LOG_LINE_LIMIT).await {
                Ok(logs) => {
                    let _ = tx.send(AppInternalEvent::LogsLoaded(logs, is_manual)).await;
                }
                Err(e) => {
                    let _ = tx
                        .send(AppInternalEvent::Error(format!("Failed to load logs: {e}")))
                        .await;
                }
            }
        });
    }

    pub async fn start_following_logs(&mut self, unit_name: String, scope: String) {
        self.stop_following_logs();
        self.log_view.is_following = true;
        self.fetch_unit_logs(unit_name.clone(), scope.clone(), false)
            .await;

        let tx = self.internal_tx.clone();
        let handle = spawn(async move {
            let manager = JournalManager::new();
            match manager.follow_logs(&unit_name, &scope, 0).await {
                Ok(mut stream) => {
                    while let Some(line) = stream.next().await {
                        let _ = tx.send(AppInternalEvent::LogLineReceived(line)).await;
                    }
                }
                Err(e) => {
                    let _ = tx
                        .send(AppInternalEvent::Error(format!(
                            "Failed to follow logs: {e}"
                        )))
                        .await;
                }
            }
        });

        self.log_view.follow_task = Some(handle);
    }

    pub fn stop_following_logs(&mut self) {
        if let Some(task) = self.log_view.follow_task.take() {
            task.abort();
        }
        self.log_view.is_following = false;
    }

    pub fn clear_log_visual_modes(&mut self) {
        self.log_view.clear_visual_modes();
    }

    pub fn log_search_matches(&self) -> Vec<usize> {
        if self.search.query.is_empty() {
            return Vec::new();
        }

        self.log_view
            .logs
            .iter()
            .enumerate()
            .filter(|(_, line)| line.contains(&self.search.query))
            .map(|(index, _)| index)
            .collect()
    }

    pub fn cycle_log_search_match(&mut self, forward: bool) {
        let matches = self.log_search_matches();
        if matches.is_empty() {
            let query = self.search.query.clone();
            self.notify(
                format!("No matches found for '{}'", query),
                crate::models::NotificationType::Error,
            );
            return;
        }

        let current = self.log_view.state.selected().unwrap_or(0);
        let next_index = if forward {
            matches
                .iter()
                .copied()
                .find(|&index| index > current)
                .unwrap_or(matches[0])
        } else {
            matches
                .iter()
                .copied()
                .rev()
                .find(|&index| index < current)
                .unwrap_or(*matches.last().unwrap())
        };

        self.log_view.state.select(Some(next_index));
        self.log_view.scroll_x = 0;
    }

    pub fn toggle_log_line_mark(&mut self) {
        self.log_view.toggle_line_mark();
    }

    pub fn selected_log_line_range(&self) -> Option<(usize, usize)> {
        self.log_view.selected_line_range()
    }

    pub fn selected_log_lines_text(&self) -> Option<String> {
        self.log_view.selected_lines_text()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;
    use zbus::zvariant::OwnedObjectPath;

    use crate::models::{UnitActiveState, UnitEnablementState, UnitInfo, UnitLoadState, UnitScope};

    fn test_app(units: Vec<UnitInfo>) -> App {
        let (tx, _rx) = mpsc::channel(1);
        let mut app = App::blank(tx);
        app.unit_list.units = units;
        app.is_loading = false;
        app
    }

    fn unit(
        name: &str,
        description: &str,
        load_state: UnitLoadState,
        active_state: UnitActiveState,
        enablement_state: UnitEnablementState,
        path: &str,
    ) -> UnitInfo {
        UnitInfo {
            name: name.to_string(),
            description: description.to_string(),
            scope: UnitScope::Global,
            load_state,
            active_state,
            enablement_state,
            sub_state: active_state.to_string(),
            path: OwnedObjectPath::try_from(path).unwrap(),
            fragment_path: format!("/etc/systemd/system/{name}"),
        }
    }

    #[test]
    fn selected_log_line_marks_keep_at_most_two_entries() {
        let mut app = test_app(vec![unit(
            "alpha.service",
            "Alpha",
            UnitLoadState::Loaded,
            UnitActiveState::Active,
            UnitEnablementState::Enabled,
            "/test/unit/alpha",
        )]);
        app.log_view.logs = vec!["one".to_string(), "two".to_string(), "three".to_string()];
        app.log_view.state.select(Some(0));

        app.toggle_log_line_mark();
        app.log_view.state.select(Some(1));
        app.toggle_log_line_mark();
        app.log_view.state.select(Some(2));
        app.toggle_log_line_mark();

        assert_eq!(app.log_view.line_marks, vec![1, 2]);
        assert_eq!(app.selected_log_line_range(), Some((1, 2)));
        assert_eq!(app.selected_log_lines_text().as_deref(), Some("two\nthree"));
    }

    #[test]
    fn log_search_helpers_match_exact_text_and_cycle() {
        let mut app = test_app(vec![unit(
            "alpha.service",
            "Alpha",
            UnitLoadState::Loaded,
            UnitActiveState::Active,
            UnitEnablementState::Enabled,
            "/test/unit/alpha",
        )]);
        app.log_view.logs = vec![
            "aaa foo aaa".to_string(),
            "foo bar".to_string(),
            "bar foo".to_string(),
            "baz".to_string(),
        ];
        app.search.query = "foo".to_string();

        assert_eq!(app.log_search_matches(), vec![0, 1, 2]);

        app.log_view.state.select(Some(0));
        app.cycle_log_search_match(true);
        assert_eq!(app.log_view.state.selected(), Some(1));

        app.cycle_log_search_match(false);
        assert_eq!(app.log_view.state.selected(), Some(0));
    }
}
