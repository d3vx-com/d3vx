//! Tests for Task Store Operations
//!
//! Covers task CRUD operations and state management.

#[cfg(test)]
mod tests {
    use crate::store::database::Database;
    use crate::store::task::{
        AgentRole, ExecutionMode, NewTask, TaskListOptions, TaskState, TaskStore, TaskUpdate,
    };

    fn create_test_db() -> Database {
        Database::in_memory().unwrap()
    }

    fn create_test_task_input(id: &str) -> NewTask {
        NewTask {
            id: Some(id.to_string()),
            title: "Test Task".to_string(),
            description: Some("A test task description".to_string()),
            state: Some(TaskState::Backlog),
            priority: Some(10),
            execution_mode: Some(ExecutionMode::Direct),
            agent_role: Some(AgentRole::Coder),
            ..Default::default()
        }
    }

    // =========================================================================
    // Task Creation Tests
    // =========================================================================

    #[test]
    fn test_task_creation() {
        let db = create_test_db();
        let store = TaskStore::new(&db);

        let new_task = create_test_task_input("task-1");
        let result = store.create(new_task);

        assert!(result.is_ok());
        let task = result.unwrap();
        assert_eq!(task.id, "task-1");
        assert_eq!(task.state, TaskState::Backlog);
    }

    #[test]
    fn test_task_with_parent() {
        let db = create_test_db();
        let store = TaskStore::new(&db);

        // Create parent task
        let parent = store.create(create_test_task_input("parent")).unwrap();

        // Create child task
        let mut child_input = create_test_task_input("child");
        child_input.parent_task_id = Some(parent.id.clone());

        let result = store.create(child_input);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().parent_task_id, Some(parent.id));
    }

    // =========================================================================
    // Task Retrieval Tests
    // =========================================================================

    #[test]
    fn test_get_task() {
        let db = create_test_db();
        let store = TaskStore::new(&db);

        let id = "task-get".to_string();
        store.create(create_test_task_input(&id)).unwrap();

        let retrieved = store.get(&id).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().title, "Test Task");
    }

    #[test]
    fn test_get_nonexistent_task() {
        let db = create_test_db();
        let store = TaskStore::new(&db);

        let result = store.get("nonexistent").unwrap();
        assert!(result.is_none());
    }

    // =========================================================================
    // Task State Tests
    // =========================================================================

    #[test]
    fn test_task_state_transitions() {
        let db = create_test_db();
        let store = TaskStore::new(&db);

        let id = "task-transition".to_string();
        store.create(create_test_task_input(&id)).unwrap();

        // Initial state should be Backlog
        let task = store.get(&id).unwrap().unwrap();
        assert_eq!(task.state, TaskState::Backlog);

        // Transition to Queued
        store.transition(&id, TaskState::Queued).unwrap();
        let task = store.get(&id).unwrap().unwrap();
        assert_eq!(task.state, TaskState::Queued);

        // Transition to Research
        store.transition(&id, TaskState::Research).unwrap();
        let task = store.get(&id).unwrap().unwrap();
        assert_eq!(task.state, TaskState::Research);
    }

    // =========================================================================
    // Task List Tests
    // =========================================================================

    #[test]
    fn test_list_tasks_empty() {
        let db = create_test_db();
        let store = TaskStore::new(&db);

        let tasks = store.list(TaskListOptions::default()).unwrap();
        assert!(tasks.is_empty());
    }

    #[test]
    fn test_list_tasks_with_filter() {
        let db = create_test_db();
        let store = TaskStore::new(&db);

        // Create tasks
        for i in 0..3 {
            store
                .create(create_test_task_input(&format!("task-{}", i)))
                .unwrap();
        }

        // Transition one to Queued
        store.transition("task-0", TaskState::Queued).unwrap();

        // Filter by Queued
        let options = TaskListOptions {
            state: Some(vec![TaskState::Queued]),
            ..Default::default()
        };
        let queued = store.list(options).unwrap();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].id, "task-0");
    }

    // =========================================================================
    // Task Update Tests
    // =========================================================================

    #[test]
    fn test_update_task_fields() {
        let db = create_test_db();
        let store = TaskStore::new(&db);

        let id = "task-update".to_string();
        store.create(create_test_task_input(&id)).unwrap();

        store
            .update(
                &id,
                TaskUpdate {
                    title: Some("Updated Title".to_string()),
                    priority: Some(100),
                    ..Default::default()
                },
            )
            .unwrap();

        let task = store.get(&id).unwrap().unwrap();
        assert_eq!(task.title, "Updated Title");
        assert_eq!(task.priority, 100);
    }

    // =========================================================================
    // Task Deletion Tests
    // =========================================================================

    #[test]
    fn test_delete_task() {
        let db = create_test_db();
        let store = TaskStore::new(&db);

        let id = "task-delete".to_string();
        store.create(create_test_task_input(&id)).unwrap();

        store.delete(&id).unwrap();

        let result = store.get(&id).unwrap();
        assert!(result.is_none());
    }
}
