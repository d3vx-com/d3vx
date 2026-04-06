//! Specialist agent types tests

use crate::agent::specialists::{AgentType, SPECIALIST_AGENT_TYPES};

#[test]
fn test_specialist_agent_types_list_has_entries() {
    assert_eq!(SPECIALIST_AGENT_TYPES.len(), 12);
}

#[test]
fn test_agent_type_display_variants() {
    assert_eq!(AgentType::General.display_name(), "Agent");
    assert_eq!(AgentType::Backend.display_name(), "Backend Dev");
    assert_eq!(AgentType::Frontend.display_name(), "Frontend Dev");
    assert_eq!(AgentType::Testing.display_name(), "QA Engineer");
    assert_eq!(AgentType::Documentation.display_name(), "Tech Writer");
    assert_eq!(AgentType::DevOps.display_name(), "DevOps Engineer");
    assert_eq!(AgentType::Security.display_name(), "Security Engineer");
    assert_eq!(AgentType::Review.display_name(), "Code Reviewer");
    assert_eq!(AgentType::Data.display_name(), "Data Engineer");
    assert_eq!(AgentType::Mobile.display_name(), "Mobile Dev");
    assert_eq!(AgentType::Explore.display_name(), "Explorer");
    assert_eq!(AgentType::Plan.display_name(), "Architect");
    assert_eq!(AgentType::Teammate.display_name(), "Swarm Mate");
}

#[test]
fn test_agent_type_specialist_role_labels() {
    assert_eq!(AgentType::General.specialist_role_label(), "Executor");
    assert_eq!(AgentType::Backend.specialist_role_label(), "Backend Specialist");
    assert_eq!(AgentType::Testing.specialist_role_label(), "Test Specialist");
    assert_eq!(AgentType::Security.specialist_role_label(), "Security Reviewer");
    assert_eq!(AgentType::Review.specialist_role_label(), "Code Reviewer");
}

#[test]
fn test_agent_type_icons() {
    assert_eq!(AgentType::General.icon(), "🤖");
    assert_eq!(AgentType::Backend.icon(), "⚙️");
    assert_eq!(AgentType::Frontend.icon(), "🎨");
    assert_eq!(AgentType::Testing.icon(), "🧪");
    assert_eq!(AgentType::DevOps.icon(), "🚀");
    assert_eq!(AgentType::Security.icon(), "🔒");
    assert_eq!(AgentType::Review.icon(), "🔍");
    assert_eq!(AgentType::Mobile.icon(), "📱");
    assert_eq!(AgentType::Teammate.icon(), "🐝");
}

#[test]
fn test_agent_type_priority_ordering() {
    assert_eq!(AgentType::General.priority(), 0);
    assert_eq!(AgentType::Teammate.priority(), 1);
    assert_eq!(AgentType::Explore.priority(), 2);
    assert_eq!(AgentType::Backend.priority(), 3);
    assert_eq!(AgentType::Testing.priority(), 4);
    assert_eq!(AgentType::Security.priority(), 5);
}

#[test]
fn test_agent_type_display_fmt() {
    assert_eq!(format!("{}", AgentType::General), "Agent");
    assert_eq!(format!("{}", AgentType::Backend), "Backend Dev");
}

#[test]
fn test_agent_type_default_is_general() {
    assert_eq!(AgentType::default(), AgentType::General);
}

#[test]
fn test_agent_type_serialization_roundtrip() {
    for agent_type in [
        AgentType::General,
        AgentType::Backend,
        AgentType::Frontend,
        AgentType::Testing,
        AgentType::DevOps,
        AgentType::Security,
        AgentType::Review,
        AgentType::Explore,
        AgentType::Plan,
        AgentType::Teammate,
    ] {
        let json = serde_json::to_string(&agent_type).unwrap();
        let parsed: AgentType = serde_json::from_str(&json).unwrap();
        assert_eq!(agent_type, parsed);
    }
}

#[test]
fn test_specialist_profile_has_all_fields() {
    for agent_type in SPECIALIST_AGENT_TYPES.iter() {
        let profile = agent_type.profile();
        assert!(!profile.display_name.is_empty());
        assert!(!profile.specialist_role_label.is_empty());
        assert!(!profile.review_focus.is_empty());
    }
}

#[test]
fn test_specialist_profile_system_prompt_additions() {
    for agent_type in SPECIALIST_AGENT_TYPES.iter() {
        let addition = agent_type.system_prompt_addition();
        // Each specialist type should have some system prompt addition (even if empty for General)
        // At minimum, it should not panic
        let _ = addition;
    }
}
