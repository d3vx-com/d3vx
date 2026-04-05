#[cfg(test)]
mod tests {
    use crate::cli::args::{parse_from, CliCommand, MemoryAction};
    use crate::cli::commands::execute_init;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_parse_default() {
        let args = parse_from(["d3vx"]);
        assert!(args.query.is_none());
        assert!(args.command.is_none());
    }

    #[test]
    fn test_parse_with_query() {
        let args = parse_from(["d3vx", "write a hello world program"]);
        assert_eq!(args.query, Some("write a hello world program".to_string()));
    }

    #[test]
    fn test_parse_init() {
        let args = parse_from(["d3vx", "init"]);
        match args.command {
            Some(CliCommand::Init { path }) => assert!(path.is_none()),
            _ => panic!("Expected Init command"),
        }
    }

    #[test]
    fn test_parse_implement() {
        let args = parse_from([
            "d3vx",
            "implement",
            "add authentication",
            "--fast",
            "--queue",
        ]);
        match args.command {
            Some(CliCommand::Implement {
                instruction,
                fast,
                quick,
                role,
                queue,
            }) => {
                assert_eq!(instruction, "add authentication");
                assert!(fast);
                assert!(!quick);
                assert!(role.is_none());
                assert!(queue);
            }
            _ => panic!("Expected Implement command"),
        }
    }

    #[test]
    fn test_parse_global_options() {
        let args = parse_from([
            "d3vx",
            "--provider",
            "openai",
            "--model",
            "gpt-4",
            "--verbose",
            "--trust",
        ]);
        assert_eq!(args.provider, Some("openai".to_string()));
        assert_eq!(args.model, Some("gpt-4".to_string()));
        assert!(args.verbose);
        assert!(args.trust);
    }

    #[tokio::test]
    async fn test_execute_init_flow() -> anyhow::Result<()> {
        let dir = tempdir().unwrap();
        let path = dir.path().to_path_buf();

        execute_init(Some(&path)).await?;

        let d3vx_dir = path.join(".d3vx");
        assert!(d3vx_dir.exists());
        assert!(d3vx_dir.join("config.yml").exists());
        assert!(d3vx_dir.join("project.md").exists());
        assert!(d3vx_dir.join("memory").is_dir());

        fs::create_dir(path.join(".git")).unwrap();

        let dir2 = tempdir().unwrap();
        let path2 = dir2.path().to_path_buf();
        fs::create_dir(path2.join(".git")).unwrap();

        execute_init(Some(&path2)).await?;
        let gitignore = fs::read_to_string(path2.join(".gitignore")).unwrap();
        assert!(gitignore.contains(".d3vx-worktrees/"));

        Ok(())
    }

    #[test]
    fn test_parse_memory_search() {
        let args = parse_from(["d3vx", "memory", "search", "api key"]);
        match args.command {
            Some(CliCommand::Memory { action }) => match action {
                MemoryAction::Search { query } => {
                    assert_eq!(query, "api key");
                }
                _ => panic!("Expected Search action"),
            },
            _ => panic!("Expected Memory command"),
        }
    }
}
