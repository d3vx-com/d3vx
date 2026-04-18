//! Tests for the declarative grader rules.

use std::fs;
use std::path::PathBuf;

use crate::evals::environment::EvalEnvironment;
use crate::evals::grader::{GradeOutcome, GraderSpec};

fn fresh_env(prefix: &str) -> EvalEnvironment {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "d3vx-evals-grader-{}-{}",
        prefix,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&p).unwrap();
    EvalEnvironment::adopt(prefix.to_string(), p)
}

fn cleanup(env: &EvalEnvironment) {
    let _ = fs::remove_dir_all(&env.workspace_path);
}

fn write(env: &EvalEnvironment, rel: &str, contents: &str) -> PathBuf {
    let path = env.workspace_path.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&path, contents).unwrap();
    path
}

#[test]
fn outcome_helpers_set_passed_flag_and_detail() {
    let ok = GradeOutcome::passed("good");
    assert!(ok.passed);
    assert_eq!(ok.detail, "good");

    let bad = GradeOutcome::failed("oops");
    assert!(!bad.passed);
    assert_eq!(bad.detail, "oops");
}

#[test]
fn file_exists_passes_when_file_present() {
    let env = fresh_env("fe_ok");
    write(&env, "hello.txt", "hi");
    let rule = GraderSpec::FileExists {
        path: "hello.txt".to_string(),
    };
    assert!(rule.grade(&env).passed);
    cleanup(&env);
}

#[test]
fn file_exists_fails_when_file_missing() {
    let env = fresh_env("fe_miss");
    let rule = GraderSpec::FileExists {
        path: "nope.txt".to_string(),
    };
    assert!(!rule.grade(&env).passed);
    cleanup(&env);
}

#[test]
fn file_contains_requires_at_least_one_constraint() {
    let env = fresh_env("fc_none");
    write(&env, "a.txt", "content");
    let rule = GraderSpec::FileContains {
        path: "a.txt".to_string(),
        substring: None,
        regex: None,
    };
    let out = rule.grade(&env);
    assert!(!out.passed);
    assert!(out.detail.contains("at least one"));
    cleanup(&env);
}

#[test]
fn file_contains_matches_substring() {
    let env = fresh_env("fc_sub");
    write(&env, "a.txt", "hello world");
    let rule = GraderSpec::FileContains {
        path: "a.txt".to_string(),
        substring: Some("world".to_string()),
        regex: None,
    };
    assert!(rule.grade(&env).passed);
    cleanup(&env);
}

#[test]
fn file_contains_fails_on_missing_substring() {
    let env = fresh_env("fc_subfail");
    write(&env, "a.txt", "hello");
    let rule = GraderSpec::FileContains {
        path: "a.txt".to_string(),
        substring: Some("absent".to_string()),
        regex: None,
    };
    assert!(!rule.grade(&env).passed);
    cleanup(&env);
}

#[test]
fn file_contains_matches_regex() {
    let env = fresh_env("fc_re");
    write(&env, "a.txt", "answer: 42");
    let rule = GraderSpec::FileContains {
        path: "a.txt".to_string(),
        substring: None,
        regex: Some(r"answer:\s*\d+".to_string()),
    };
    assert!(rule.grade(&env).passed);
    cleanup(&env);
}

#[test]
fn file_contains_reports_invalid_regex_as_failure() {
    let env = fresh_env("fc_badre");
    write(&env, "a.txt", "x");
    let rule = GraderSpec::FileContains {
        path: "a.txt".to_string(),
        substring: None,
        regex: Some("[invalid".to_string()),
    };
    let out = rule.grade(&env);
    assert!(!out.passed);
    assert!(out.detail.contains("invalid regex"));
    cleanup(&env);
}

#[test]
fn file_contains_requires_both_constraints_when_both_given() {
    let env = fresh_env("fc_both");
    write(&env, "a.txt", "hello world");
    // substring matches, regex doesn't → overall fail
    let rule = GraderSpec::FileContains {
        path: "a.txt".to_string(),
        substring: Some("hello".to_string()),
        regex: Some(r"\d+".to_string()),
    };
    assert!(!rule.grade(&env).passed);
    cleanup(&env);
}

#[test]
fn shell_command_passes_on_exit_zero_by_default() {
    let env = fresh_env("sh_ok");
    let rule = GraderSpec::ShellCommand {
        command: "true".to_string(),
        pass_on_exit_zero: true,
    };
    assert!(rule.grade(&env).passed);
    cleanup(&env);
}

#[test]
fn shell_command_fails_on_non_zero_by_default() {
    let env = fresh_env("sh_bad");
    let rule = GraderSpec::ShellCommand {
        command: "false".to_string(),
        pass_on_exit_zero: true,
    };
    assert!(!rule.grade(&env).passed);
    cleanup(&env);
}

#[test]
fn shell_command_can_pass_on_non_zero() {
    // Negative-assertion use case: "the build should still fail here".
    let env = fresh_env("sh_neg");
    let rule = GraderSpec::ShellCommand {
        command: "false".to_string(),
        pass_on_exit_zero: false,
    };
    assert!(rule.grade(&env).passed);
    cleanup(&env);
}

#[test]
fn shell_command_runs_in_workspace_dir() {
    let env = fresh_env("sh_cwd");
    write(&env, "marker.txt", "here");
    let rule = GraderSpec::ShellCommand {
        command: "test -f marker.txt".to_string(),
        pass_on_exit_zero: true,
    };
    assert!(rule.grade(&env).passed);
    cleanup(&env);
}

#[test]
fn all_passes_when_every_child_passes() {
    let env = fresh_env("all_ok");
    write(&env, "a.txt", "x");
    let rule = GraderSpec::All {
        graders: vec![
            GraderSpec::FileExists {
                path: "a.txt".to_string(),
            },
            GraderSpec::ShellCommand {
                command: "true".to_string(),
                pass_on_exit_zero: true,
            },
        ],
    };
    assert!(rule.grade(&env).passed);
    cleanup(&env);
}

#[test]
fn all_fails_and_reports_first_offender() {
    let env = fresh_env("all_bad");
    write(&env, "a.txt", "x");
    let rule = GraderSpec::All {
        graders: vec![
            GraderSpec::FileExists {
                path: "a.txt".to_string(),
            },
            GraderSpec::FileExists {
                path: "missing.txt".to_string(),
            },
            GraderSpec::ShellCommand {
                command: "true".to_string(),
                pass_on_exit_zero: true,
            },
        ],
    };
    let out = rule.grade(&env);
    assert!(!out.passed);
    assert!(out.detail.contains("missing.txt"));
    cleanup(&env);
}

#[test]
fn any_passes_if_one_child_passes() {
    let env = fresh_env("any_ok");
    let rule = GraderSpec::Any {
        graders: vec![
            GraderSpec::FileExists {
                path: "nope.txt".to_string(),
            },
            GraderSpec::ShellCommand {
                command: "true".to_string(),
                pass_on_exit_zero: true,
            },
        ],
    };
    assert!(rule.grade(&env).passed);
    cleanup(&env);
}

#[test]
fn any_fails_when_all_fail_and_lists_reasons() {
    let env = fresh_env("any_bad");
    let rule = GraderSpec::Any {
        graders: vec![
            GraderSpec::FileExists {
                path: "a.txt".to_string(),
            },
            GraderSpec::FileExists {
                path: "b.txt".to_string(),
            },
        ],
    };
    let out = rule.grade(&env);
    assert!(!out.passed);
    assert!(out.detail.contains("a.txt"));
    assert!(out.detail.contains("b.txt"));
    cleanup(&env);
}

#[test]
fn describe_produces_useful_strings_for_logs() {
    let rules = vec![
        GraderSpec::FileExists {
            path: "a".to_string(),
        },
        GraderSpec::ShellCommand {
            command: "cargo test".to_string(),
            pass_on_exit_zero: true,
        },
        GraderSpec::FileContains {
            path: "a".to_string(),
            substring: Some("foo".to_string()),
            regex: None,
        },
        GraderSpec::All { graders: vec![] },
        GraderSpec::Any { graders: vec![] },
    ];
    for r in rules {
        let d = r.describe();
        assert!(!d.is_empty());
    }
}
