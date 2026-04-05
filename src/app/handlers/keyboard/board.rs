//! Board (kanban) mode keyboard handling

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{App, AppMode};

impl App {
    pub(crate) fn handle_board_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.ui.mode = AppMode::Chat;
            }
            KeyCode::Up | KeyCode::Char('k') => self.board.select_up(),
            KeyCode::Down | KeyCode::Char('j') => self.board.select_down(),
            KeyCode::Left | KeyCode::Char('h') => self.board.select_left(),
            KeyCode::Right | KeyCode::Char('l') => self.board.select_right(),
            KeyCode::Enter => {
                if let Some(task) = self.board.selected_task() {
                    if let Some(index) = self.workspaces.iter().position(|ws| ws.id == task.id) {
                        self.switch_workspace(index)?;
                        self.ui.mode = AppMode::Chat;
                    }
                }
            }
            KeyCode::Char('H') => {
                if let Some(task) = self.board.selected_task().cloned() {
                    let cols = crate::ui::widgets::board::KanbanColumn::STANDARD_COLUMNS;
                    if let Some(current_col) =
                        cols.iter().position(|c| c.contains_state(&task.state))
                    {
                        if current_col > 0 {
                            let new_state = cols[current_col - 1].states[0];
                            if let Some(ref db_handle) = self.db {
                                let db = db_handle.lock();
                                let store = crate::store::task::TaskStore::new(&db);
                                let _ = store.update(
                                    &task.id,
                                    crate::store::task::TaskUpdate {
                                        state: Some(new_state),
                                        ..Default::default()
                                    },
                                );
                            }
                            // Force refresh next tick
                            self.last_workspace_refresh =
                                std::time::Instant::now() - std::time::Duration::from_secs(60);
                        }
                    }
                }
            }
            KeyCode::Char('L') => {
                if let Some(task) = self.board.selected_task().cloned() {
                    let cols = crate::ui::widgets::board::KanbanColumn::STANDARD_COLUMNS;
                    if let Some(current_col) =
                        cols.iter().position(|c| c.contains_state(&task.state))
                    {
                        if current_col + 1 < cols.len() {
                            let new_state = cols[current_col + 1].states[0];
                            if let Some(ref db_handle) = self.db {
                                let db = db_handle.lock();
                                let store = crate::store::task::TaskStore::new(&db);
                                let _ = store.update(
                                    &task.id,
                                    crate::store::task::TaskUpdate {
                                        state: Some(new_state),
                                        ..Default::default()
                                    },
                                );
                            }
                            // Force refresh next tick
                            self.last_workspace_refresh =
                                std::time::Instant::now() - std::time::Duration::from_secs(60);
                        }
                    }
                }
            }
            KeyCode::Char('a') => {
                if self.board.selected_col == 0 {
                    self.ui.mode = AppMode::Chat;
                    self.ui.input_buffer.clear();
                    self.ui.input_buffer.push_str("/task new ");
                    self.ui.cursor_position = self.ui.input_buffer.len();
                }
            }
            _ => {}
        }
        Ok(())
    }
}
