//! Core Application Actions

pub mod git;
pub mod mentions;
pub mod message;
pub mod shell;
pub mod tools;
pub mod workspaces;
pub mod worktree;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MessageExecutionFlags {
    pub vex: bool,
    pub review: bool,
    pub merge: bool,
    pub docs: bool,
    pub parallel: bool,
}

impl MessageExecutionFlags {
    pub fn requires_background_task(&self) -> bool {
        self.vex || self.review || self.merge || self.docs
    }

    pub fn review_required(&self) -> bool {
        self.review || self.merge
    }

    pub fn parallel_agents(&self) -> bool {
        self.parallel
    }
}

pub fn parse_message_execution_flags(content: &str) -> (String, MessageExecutionFlags) {
    let mut flags = MessageExecutionFlags::default();
    let mut remaining = Vec::new();

    for token in content.split_whitespace() {
        match token {
            "--vex" => flags.vex = true,
            "--review" => flags.review = true,
            "--merge" => flags.merge = true,
            "--docs" => flags.docs = true,
            "--parallel" => flags.parallel = true,
            _ => remaining.push(token),
        }
    }

    let description = remaining.join(" ").trim().to_string();
    if !flags.parallel && implies_parallel_agent_intent(&description) {
        flags.parallel = true;
    }

    (description, flags)
}

fn implies_parallel_agent_intent(content: &str) -> bool {
    let normalized = content.to_lowercase();
    [
        "parallel agent",
        "parallel agents",
        "using parallel agents",
        "use parallel agents",
        "multi agent",
        "multi-agent",
        "multiple agents",
        "split this into parallel",
        "analyze this codebase using parallel agents",
    ]
    .iter()
    .any(|pattern| normalized.contains(pattern))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_chat_execution_flags() {
        let (description, flags) =
            parse_message_execution_flags("implement auth retry logic --vex --review --merge");

        assert_eq!(description, "implement auth retry logic");
        assert!(flags.vex);
        assert!(flags.review);
        assert!(flags.merge);
        assert!(!flags.docs);
        assert!(flags.requires_background_task());
        assert!(flags.review_required());
    }

    #[test]
    fn parses_docs_flag() {
        let (description, flags) =
            parse_message_execution_flags("write release notes --vex --docs");

        assert_eq!(description, "write release notes");
        assert!(flags.vex);
        assert!(flags.docs);
        assert!(flags.requires_background_task());
    }

    #[test]
    fn infers_parallel_flag_from_natural_language_request() {
        let (description, flags) = parse_message_execution_flags(
            "Act as a senior developer and analyze what the codebase is all about using parallel agents",
        );

        assert_eq!(
            description,
            "Act as a senior developer and analyze what the codebase is all about using parallel agents"
        );
        assert!(flags.parallel);
    }
}
