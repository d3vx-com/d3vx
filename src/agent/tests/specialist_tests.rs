use crate::agent::specialists::AgentType;
use crate::tools::AgentRole;

#[test]
fn specialist_profiles_expose_expected_roles() {
    assert_eq!(
        AgentType::Backend.profile().recommended_role,
        AgentRole::Executor
    );
    assert_eq!(
        AgentType::Review.profile().recommended_role,
        AgentRole::QaEngineer
    );
    assert_eq!(
        AgentType::Security.profile().recommended_role,
        AgentRole::QaEngineer
    );
}

#[test]
fn specialist_profiles_expose_skill_slugs() {
    assert_eq!(
        AgentType::Documentation.profile().suggested_skill_slug,
        Some("documentation")
    );
    assert_eq!(AgentType::General.profile().suggested_skill_slug, None);
}
