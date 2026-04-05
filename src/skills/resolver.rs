//! Skill Dependency Resolver
//!
//! Resolves skill dependencies and provides topological ordering.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use super::types::Skill;

#[derive(Debug, thiserror::Error)]
pub enum ResolverError {
    #[error("Circular dependency detected: {0}")]
    CircularDependency(String),
}

pub struct SkillResolver;

impl SkillResolver {
    pub fn new() -> Self {
        Self
    }

    pub fn resolve_order(&self, skills: &[Skill]) -> Result<Vec<Arc<Skill>>, ResolverError> {
        let mut graph: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        let mut skill_map: HashMap<&str, Arc<Skill>> = HashMap::new();

        for skill in skills {
            let name = skill.name.as_str();
            skill_map.insert(name, Arc::new(skill.clone()));
            graph.entry(name).or_default();
            let _ = *in_degree.entry(name).or_insert(0);
        }

        for skill in skills {
            let name = skill.name.as_str();
            for dep in &skill.depends_on {
                if let Some(neighbors) = graph.get_mut(dep.as_str()) {
                    if !neighbors.contains(&name) {
                        neighbors.push(name);
                    }
                }
                if skill_map.contains_key(dep.as_str()) {
                    *in_degree.entry(name).or_insert(0) += 1;
                }
            }
        }

        let mut queue: VecDeque<&str> = in_degree
            .iter()
            .filter(|(_, &d)| d == 0)
            .map(|(k, _)| *k)
            .collect();

        let mut sorted = Vec::with_capacity(skills.len());

        while let Some(node) = queue.pop_front() {
            if sorted.iter().any(|s: &Arc<Skill>| s.name.as_str() == node) {
                continue;
            }
            if let Some(skill) = skill_map.get(node) {
                sorted.push(skill.clone());
            }

            if let Some(neighbors) = graph.get(node) {
                for &neighbor in neighbors {
                    if let Some(degree) = in_degree.get_mut(neighbor) {
                        *degree = degree.saturating_sub(1);
                        if *degree == 0 {
                            queue.push_back(neighbor);
                        }
                    }
                }
            }
        }

        if sorted.len() != skills.len() {
            let sorted_names: HashSet<&str> = sorted.iter().map(|s| s.name.as_str()).collect();
            let cycle: Vec<String> = skills
                .iter()
                .filter(|s| !sorted_names.contains(s.name.as_str()))
                .map(|s| s.name.clone())
                .collect();
            return Err(ResolverError::CircularDependency(cycle.join(" -> ")));
        }

        Ok(sorted)
    }
}

impl Default for SkillResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_skill(name: &str, deps: Vec<String>) -> Skill {
        Skill {
            name: name.to_string(),
            description: format!("Skill {}", name),
            triggers: vec![],
            content: format!("Content for {}", name),
            tools: vec![],
            source_path: None,
            enabled: true,
            depends_on: deps,
        }
    }

    #[test]
    fn test_no_dependencies() {
        let resolver = SkillResolver::new();
        let skills = vec![
            make_skill("a", vec![]),
            make_skill("b", vec![]),
            make_skill("c", vec![]),
        ];
        let sorted = resolver.resolve_order(&skills).unwrap();
        assert_eq!(sorted.len(), 3);
    }

    #[test]
    fn test_linear_dependency() {
        let resolver = SkillResolver::new();
        let skills = vec![
            make_skill("a", vec![]),
            make_skill("b", vec!["a".to_string()]),
            make_skill("c", vec!["b".to_string()]),
        ];
        let sorted = resolver.resolve_order(&skills).unwrap();
        let names: Vec<&str> = sorted.iter().map(|s| s.name.as_str()).collect();
        let a_pos = names.iter().position(|&n| n == "a").unwrap();
        let b_pos = names.iter().position(|&n| n == "b").unwrap();
        let c_pos = names.iter().position(|&n| n == "c").unwrap();
        assert!(a_pos < b_pos);
        assert!(b_pos < c_pos);
    }

    #[test]
    fn test_circular_dependency() {
        let resolver = SkillResolver::new();
        let skills = vec![
            make_skill("a", vec!["c".to_string()]),
            make_skill("b", vec!["a".to_string()]),
            make_skill("c", vec!["b".to_string()]),
        ];
        let result = resolver.resolve_order(&skills);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ResolverError::CircularDependency(_)
        ));
    }

    #[test]
    fn test_diamond_dependency() {
        let resolver = SkillResolver::new();
        let skills = vec![
            make_skill("base", vec![]),
            make_skill("left", vec!["base".to_string()]),
            make_skill("right", vec!["base".to_string()]),
            make_skill("top", vec!["left".to_string(), "right".to_string()]),
        ];
        let sorted = resolver.resolve_order(&skills).unwrap();
        let names: Vec<&str> = sorted.iter().map(|s| s.name.as_str()).collect();
        let base_pos = names.iter().position(|&n| n == "base").unwrap();
        let left_pos = names.iter().position(|&n| n == "left").unwrap();
        let right_pos = names.iter().position(|&n| n == "right").unwrap();
        let top_pos = names.iter().position(|&n| n == "top").unwrap();
        assert!(base_pos < left_pos);
        assert!(base_pos < right_pos);
        assert!(left_pos < top_pos);
        assert!(right_pos < top_pos);
    }
}
