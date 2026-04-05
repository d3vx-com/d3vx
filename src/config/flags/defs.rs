//! Feature Flag Definitions
//!
//! All feature flags with descriptions and categories.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureCategory {
    Agent,
    Tools,
    UI,
    Pipeline,
    Security,
    Experimental,
}

pub struct FeatureFlag {
    pub name: &'static str,
    pub category: FeatureCategory,
    pub description: &'static str,
    pub default_enabled: bool,
}

pub const ALL_FLAGS: &[FeatureFlag] = &[
    // Agent
    FeatureFlag {
        name: "auto_compact",
        category: FeatureCategory::Agent,
        description: "Auto-compact context when approaching limits",
        default_enabled: true,
    },
    FeatureFlag {
        name: "doom_loop_detection",
        category: FeatureCategory::Agent,
        description: "Detect and prevent infinite tool loops",
        default_enabled: true,
    },
    FeatureFlag {
        name: "step_controller",
        category: FeatureCategory::Agent,
        description: "Programmatic execution control",
        default_enabled: false,
    },
    // Tools
    FeatureFlag {
        name: "read_before_write",
        category: FeatureCategory::Tools,
        description: "Read file before writing",
        default_enabled: true,
    },
    FeatureFlag {
        name: "web_search",
        category: FeatureCategory::Tools,
        description: "Enable web search tool",
        default_enabled: true,
    },
    FeatureFlag {
        name: "bash_classifier",
        category: FeatureCategory::Tools,
        description: "Classify bash commands by safety",
        default_enabled: true,
    },
    // UI
    FeatureFlag {
        name: "kanban_view",
        category: FeatureCategory::UI,
        description: "Kanban board UI mode",
        default_enabled: true,
    },
    FeatureFlag {
        name: "markdown_rendering",
        category: FeatureCategory::UI,
        description: "Render markdown in output",
        default_enabled: true,
    },
    // Pipeline
    FeatureFlag {
        name: "pipeline_stages",
        category: FeatureCategory::Pipeline,
        description: "Enable 6-phase pipeline",
        default_enabled: true,
    },
    FeatureFlag {
        name: "background_tasks",
        category: FeatureCategory::Pipeline,
        description: "Background task processing",
        default_enabled: true,
    },
    FeatureFlag {
        name: "worktree_isolation",
        category: FeatureCategory::Pipeline,
        description: "Git worktree isolation per task",
        default_enabled: true,
    },
    // Security
    FeatureFlag {
        name: "sandbox_enabled",
        category: FeatureCategory::Security,
        description: "Sandbox command execution",
        default_enabled: false,
    },
    FeatureFlag {
        name: "permission_system",
        category: FeatureCategory::Security,
        description: "Permission checking before tool execution",
        default_enabled: true,
    },
    // Experimental
    FeatureFlag {
        name: "sdk_mode",
        category: FeatureCategory::Experimental,
        description: "NDJSON SDK mode for programmatic use",
        default_enabled: false,
    },
    FeatureFlag {
        name: "transport_layer",
        category: FeatureCategory::Experimental,
        description: "Pluggable transport abstraction",
        default_enabled: false,
    },
    FeatureFlag {
        name: "auto_review",
        category: FeatureCategory::Experimental,
        description: "Post-edit code review",
        default_enabled: true,
    },
];

impl FeatureFlag {
    pub fn by_name(name: &str) -> Option<&'static FeatureFlag> {
        ALL_FLAGS.iter().find(|f| f.name == name)
    }
}
