//! List mode keyboard handling

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{App, AppMode};

impl App {
    pub(crate) fn handle_list_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.ui.mode = AppMode::Chat;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.list_selected_task > 0 {
                    self.list_selected_task -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.list_selected_task + 1 < self.task_view_tasks.len() {
                    self.list_selected_task += 1;
                }
            }
            KeyCode::Enter => {
                if let Some(task) = self.task_view_tasks.get(self.list_selected_task) {
                    if let Some(index) = self.workspaces.iter().position(|ws| ws.id == task.id) {
                        self.switch_workspace(index)?;
                        self.ui.mode = AppMode::Chat;
                    }
                }
            }
            KeyCode::Char(' ') => {
                if let Some(task) = self.task_view_tasks.get(self.list_selected_task).cloned() {
                    let new_state = if task.state == crate::store::task::TaskState::Done {
                        crate::store::task::TaskState::Queued
                    } else {
                        crate::store::task::TaskState::Done
                    };

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

                    if let Some(t) = self.task_view_tasks.get_mut(self.list_selected_task) {
                        t.state = new_state;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}
