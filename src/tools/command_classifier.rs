//! Command Classifier
//!
//! Classifies shell commands into safety categories for permission auto-approval.
//! Safe commands (read-only, no side effects) can be auto-approved.

use once_cell::sync::Lazy;
use std::collections::HashSet;

/// Safety classification for a shell command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandSafety {
    /// Read-only commands safe to auto-approve (ls, cat, git status, etc.)
    Safe,
    /// Commands that modify files or state -- require approval
    Unsafe,
    /// Ambiguous commands that need closer inspection
    Ambiguous,
}

/// Simple read-only commands (no subcommand logic needed).
static SAFE_COMMANDS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    HashSet::from([
        "ls", "cat", "head", "tail", "pwd", "which", "echo", "wc", "find", "env", "printenv",
        "whoami", "hostname", "uname", "date", "df", "du", "file", "stat", "tree", "rg", "grep",
        "ag", "fd",
    ])
});

/// Commands that are always unsafe (modify state/filesystem).
static UNSAFE_COMMANDS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    HashSet::from([
        "rm", "rmdir", "mv", "cp", "chmod", "chown", "sudo", "wget", "kill", "killall", "shutdown",
        "reboot", "mkfs", "dd", "format",
    ])
});

/// Extract the base command name from a command string.
///
/// Strips leading whitespace, resolves the final path component,
/// and returns the bare command name (e.g., `/usr/bin/git` -> `git`).
pub fn extract_base_command(command: &str) -> String {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    // Take only the first token (the command itself, before any flags/args).
    let first_token = trimmed.split_whitespace().next().unwrap_or(trimmed);
    // Strip directory path if present (e.g., /usr/bin/git -> git).
    first_token
        .rsplit('/')
        .next()
        .unwrap_or(first_token)
        .to_string()
}

/// Classify a git subcommand by safety.
fn classify_git(subcommand: &str) -> CommandSafety {
    let safe_git = [
        "status",
        "log",
        "diff",
        "show",
        "remote",
        "stash",
        "branch",
        "tag",
        "describe",
        "rev-parse",
        "ls-files",
        "ls-remote",
    ];
    let unsafe_git = [
        "add",
        "commit",
        "push",
        "reset",
        "checkout",
        "rebase",
        "merge",
        "stash pop",
        "stash drop",
        "clean",
        "rm",
        "mv",
        "init",
        "clone",
        "fetch",
        "pull",
        "cherry-pick",
        "revert",
        "am",
        "apply",
        "bisect",
    ];

    let sub_lower = subcommand.to_lowercase();

    // Check stash sub-commands: "stash list" is safe, "stash pop/drop" is unsafe.
    if sub_lower.starts_with("stash") {
        if sub_lower == "stash" || sub_lower == "stash list" {
            return CommandSafety::Safe;
        }
        return CommandSafety::Unsafe;
    }

    for safe in &safe_git {
        if sub_lower.starts_with(safe) {
            return CommandSafety::Safe;
        }
    }

    for unsafe_cmd in &unsafe_git {
        if sub_lower.starts_with(unsafe_cmd) {
            return CommandSafety::Unsafe;
        }
    }

    CommandSafety::Ambiguous
}

/// Classify a cargo subcommand by safety.
fn classify_cargo(subcommand: &str) -> CommandSafety {
    let safe_cargo = [
        "check",
        "clippy",
        "--version",
        "-v",
        "locate-project",
        "metadata",
    ];
    let sub_lower = subcommand.to_lowercase();

    for safe in &safe_cargo {
        if sub_lower.starts_with(safe) {
            return CommandSafety::Safe;
        }
    }

    // cargo test --no-run and cargo test --no-run are still build steps,
    // but `cargo test` executes code and writes to the filesystem.
    // We treat cargo build/run/test as unsafe since they write artifacts.
    let unsafe_cargo = [
        "build", "run", "test", "install", "clean", "publish", "bench",
    ];
    for unsafe_cmd in &unsafe_cargo {
        if sub_lower.starts_with(unsafe_cmd) {
            return CommandSafety::Unsafe;
        }
    }

    CommandSafety::Ambiguous
}

/// Classify a single command segment (no pipes/redirects) into a safety level.
fn classify_single(command: &str) -> CommandSafety {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return CommandSafety::Ambiguous;
    }

    let base = extract_base_command(trimmed);
    let base_lower = base.to_lowercase();

    // Check simple unsafe commands first.
    if UNSAFE_COMMANDS.contains(base_lower.as_str()) {
        return CommandSafety::Unsafe;
    }

    // Check simple safe commands.
    if SAFE_COMMANDS.contains(base_lower.as_str()) {
        return CommandSafety::Safe;
    }

    // Handle `curl` specially: `curl | sh` is unsafe, plain curl is ambiguous.
    if base_lower == "curl" {
        return CommandSafety::Ambiguous;
    }

    // Handle `pip install` / `npm install -g` as unsafe.
    if base_lower == "pip" {
        let rest = trimmed.split_whitespace().nth(1).unwrap_or("");
        if rest == "install" {
            return CommandSafety::Unsafe;
        }
        return CommandSafety::Ambiguous;
    }

    if base_lower == "npm" {
        let tokens: Vec<&str> = trimmed.split_whitespace().collect();
        if tokens.len() >= 2 && tokens[1] == "install" {
            // npm install -g or npm install --global is unsafe
            if tokens.len() >= 3 && (tokens[2] == "-g" || tokens[2] == "--global") {
                return CommandSafety::Unsafe;
            }
            // Plain npm install (local) is also unsafe as it modifies node_modules.
            return CommandSafety::Unsafe;
        }
        if tokens.len() >= 2 && tokens[1] == "list" {
            return CommandSafety::Safe;
        }
        if tokens.len() >= 2 && tokens[1] == "--version" {
            return CommandSafety::Safe;
        }
        return CommandSafety::Ambiguous;
    }

    if base_lower == "node" {
        let tokens: Vec<&str> = trimmed.split_whitespace().collect();
        if tokens.len() >= 2 && tokens[1].starts_with("--version") {
            return CommandSafety::Safe;
        }
        return CommandSafety::Ambiguous;
    }

    if base_lower == "python" || base_lower == "python3" {
        let tokens: Vec<&str> = trimmed.split_whitespace().collect();
        if tokens.len() >= 2 && tokens[1].starts_with("--version") {
            return CommandSafety::Safe;
        }
        return CommandSafety::Ambiguous;
    }

    if base_lower == "go" {
        let tokens: Vec<&str> = trimmed.split_whitespace().collect();
        if tokens.len() >= 2 && tokens[1] == "vet" {
            return CommandSafety::Safe;
        }
        return CommandSafety::Ambiguous;
    }

    if base_lower == "docker" {
        let tokens: Vec<&str> = trimmed.split_whitespace().collect();
        if tokens.len() >= 2 {
            match tokens[1] {
                "rm" | "rmi" | "stop" | "kill" | "prune" | "system" => {
                    return CommandSafety::Unsafe;
                }
                "ps" | "images" | "logs" | "inspect" | "version" => {
                    return CommandSafety::Safe;
                }
                _ => {}
            }
        }
        return CommandSafety::Ambiguous;
    }

    // Handle `rustfmt --check` as safe.
    if base_lower == "rustfmt" {
        let tokens: Vec<&str> = trimmed.split_whitespace().collect();
        if tokens.len() >= 2 && tokens[1] == "--check" {
            return CommandSafety::Safe;
        }
        return CommandSafety::Ambiguous;
    }

    // Handle git subcommands.
    if base_lower == "git" {
        let tokens: Vec<&str> = trimmed.split_whitespace().collect();
        if tokens.len() >= 2 {
            // Reconstruct the subcommand and its argument for compound checks (e.g., "stash list").
            let sub_and_arg: String = tokens[2..].join(" ");
            let sub_cmd = if sub_and_arg.is_empty() {
                tokens[1].to_string()
            } else {
                format!("{} {}", tokens[1], sub_and_arg)
            };
            return classify_git(&sub_cmd);
        }
        // Bare `git` with no subcommand is ambiguous.
        return CommandSafety::Ambiguous;
    }

    // Handle cargo subcommands.
    if base_lower == "cargo" {
        let tokens: Vec<&str> = trimmed.split_whitespace().collect();
        if tokens.len() >= 2 {
            let sub_and_rest: String = tokens[1..].join(" ");
            return classify_cargo(&sub_and_rest);
        }
        // Bare `cargo` is ambiguous.
        return CommandSafety::Ambiguous;
    }

    CommandSafety::Ambiguous
}

/// Classify a shell command into its safety category.
///
/// Handles pipe chains (takes the most unsafe classification) and
/// output redirects (always unsafe).
pub fn classify_command(command: &str) -> CommandSafety {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return CommandSafety::Ambiguous;
    }

    // Output redirects make any command unsafe.
    if trimmed.contains('>') {
        return CommandSafety::Unsafe;
    }

    // Split on pipe to classify each segment independently.
    let segments: Vec<&str> = trimmed.split('|').collect();

    let mut result = CommandSafety::Safe;
    for segment in &segments {
        let seg_safety = classify_single(segment);
        result = more_unsafe(result, seg_safety);
    }

    result
}

/// Return the more unsafe of two safety classifications.
fn more_unsafe(a: CommandSafety, b: CommandSafety) -> CommandSafety {
    match (a, b) {
        (CommandSafety::Unsafe, _) | (_, CommandSafety::Unsafe) => CommandSafety::Unsafe,
        (CommandSafety::Ambiguous, _) | (_, CommandSafety::Ambiguous) => CommandSafety::Ambiguous,
        _ => CommandSafety::Safe,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- extract_base_command tests --

    #[test]
    fn test_extract_base_command_simple() {
        assert_eq!(extract_base_command("ls"), "ls");
        assert_eq!(extract_base_command("git status"), "git");
    }

    #[test]
    fn test_extract_base_command_with_path() {
        assert_eq!(extract_base_command("/usr/bin/git status"), "git");
        assert_eq!(extract_base_command("./cargo test"), "cargo");
    }

    #[test]
    fn test_extract_base_command_with_whitespace() {
        assert_eq!(extract_base_command("  ls -la"), "ls");
        assert_eq!(extract_base_command("\tcat file.txt"), "cat");
    }

    #[test]
    fn test_extract_base_command_empty() {
        assert_eq!(extract_base_command(""), "");
        assert_eq!(extract_base_command("   "), "");
    }

    // -- Safe command tests --

    #[test]
    fn test_safe_simple_commands() {
        assert_eq!(classify_command("ls"), CommandSafety::Safe);
        assert_eq!(classify_command("cat file.txt"), CommandSafety::Safe);
        assert_eq!(classify_command("head -n 10 file.rs"), CommandSafety::Safe);
        assert_eq!(classify_command("tail -f log.txt"), CommandSafety::Safe);
        assert_eq!(classify_command("pwd"), CommandSafety::Safe);
        assert_eq!(classify_command("which cargo"), CommandSafety::Safe);
        assert_eq!(classify_command("echo hello"), CommandSafety::Safe);
        assert_eq!(classify_command("wc -l file.rs"), CommandSafety::Safe);
        assert_eq!(classify_command("find . -name '*.rs'"), CommandSafety::Safe);
    }

    #[test]
    fn test_safe_env_commands() {
        assert_eq!(classify_command("env"), CommandSafety::Safe);
        assert_eq!(classify_command("printenv PATH"), CommandSafety::Safe);
        assert_eq!(classify_command("whoami"), CommandSafety::Safe);
        assert_eq!(classify_command("hostname"), CommandSafety::Safe);
        assert_eq!(classify_command("uname -a"), CommandSafety::Safe);
        assert_eq!(classify_command("date"), CommandSafety::Safe);
    }

    #[test]
    fn test_safe_disk_commands() {
        assert_eq!(classify_command("df -h"), CommandSafety::Safe);
        assert_eq!(classify_command("du -sh ."), CommandSafety::Safe);
        assert_eq!(classify_command("file main.rs"), CommandSafety::Safe);
        assert_eq!(classify_command("stat config.yml"), CommandSafety::Safe);
        assert_eq!(classify_command("tree src/"), CommandSafety::Safe);
    }

    #[test]
    fn test_safe_search_commands() {
        assert_eq!(classify_command("rg 'pattern' src/"), CommandSafety::Safe);
        assert_eq!(classify_command("grep -r 'todo' ."), CommandSafety::Safe);
        assert_eq!(classify_command("ag 'function' ."), CommandSafety::Safe);
        assert_eq!(classify_command("fd '.rs$'"), CommandSafety::Safe);
    }

    // -- Safe git commands --

    #[test]
    fn test_safe_git_commands() {
        assert_eq!(classify_command("git status"), CommandSafety::Safe);
        assert_eq!(classify_command("git log --oneline"), CommandSafety::Safe);
        assert_eq!(classify_command("git diff HEAD"), CommandSafety::Safe);
        assert_eq!(classify_command("git branch"), CommandSafety::Safe);
        assert_eq!(classify_command("git show HEAD"), CommandSafety::Safe);
        assert_eq!(classify_command("git remote -v"), CommandSafety::Safe);
        assert_eq!(classify_command("git stash list"), CommandSafety::Safe);
        assert_eq!(classify_command("git tag"), CommandSafety::Safe);
        assert_eq!(classify_command("git describe"), CommandSafety::Safe);
        assert_eq!(classify_command("git rev-parse HEAD"), CommandSafety::Safe);
        assert_eq!(classify_command("git ls-files"), CommandSafety::Safe);
    }

    // -- Safe cargo commands --

    #[test]
    fn test_safe_cargo_commands() {
        assert_eq!(classify_command("cargo check"), CommandSafety::Safe);
        assert_eq!(classify_command("cargo clippy"), CommandSafety::Safe);
        assert_eq!(classify_command("cargo --version"), CommandSafety::Safe);
        assert_eq!(classify_command("cargo -V"), CommandSafety::Safe);
    }

    // -- Safe other tool commands --

    #[test]
    fn test_safe_tool_version_commands() {
        assert_eq!(classify_command("npm list"), CommandSafety::Safe);
        assert_eq!(classify_command("node --version"), CommandSafety::Safe);
        assert_eq!(classify_command("python --version"), CommandSafety::Safe);
        assert_eq!(classify_command("python3 --version"), CommandSafety::Safe);
        assert_eq!(classify_command("rustfmt --check"), CommandSafety::Safe);
        assert_eq!(classify_command("go vet ./..."), CommandSafety::Safe);
    }

    // -- Unsafe command tests --

    #[test]
    fn test_unsafe_simple_commands() {
        assert_eq!(classify_command("rm file.txt"), CommandSafety::Unsafe);
        assert_eq!(classify_command("rmdir dir/"), CommandSafety::Unsafe);
        assert_eq!(classify_command("mv a.txt b.txt"), CommandSafety::Unsafe);
        assert_eq!(classify_command("cp src dst"), CommandSafety::Unsafe);
        assert_eq!(
            classify_command("chmod 755 script.sh"),
            CommandSafety::Unsafe
        );
        assert_eq!(classify_command("chown user file"), CommandSafety::Unsafe);
        assert_eq!(classify_command("sudo apt install"), CommandSafety::Unsafe);
    }

    #[test]
    fn test_unsafe_network_commands() {
        assert_eq!(
            classify_command("wget http://example.com"),
            CommandSafety::Unsafe
        );
    }

    #[test]
    fn test_unsafe_system_commands() {
        assert_eq!(classify_command("kill 1234"), CommandSafety::Unsafe);
        assert_eq!(classify_command("killall node"), CommandSafety::Unsafe);
        assert_eq!(classify_command("shutdown -h now"), CommandSafety::Unsafe);
        assert_eq!(classify_command("reboot"), CommandSafety::Unsafe);
        assert_eq!(classify_command("mkfs /dev/sda1"), CommandSafety::Unsafe);
        assert_eq!(
            classify_command("dd if=/dev/zero of=/dev/sda"),
            CommandSafety::Unsafe
        );
    }

    // -- Unsafe git commands --

    #[test]
    fn test_unsafe_git_commands() {
        assert_eq!(classify_command("git add ."), CommandSafety::Unsafe);
        assert_eq!(
            classify_command("git commit -m 'msg'"),
            CommandSafety::Unsafe
        );
        assert_eq!(classify_command("git push"), CommandSafety::Unsafe);
        assert_eq!(classify_command("git reset --hard"), CommandSafety::Unsafe);
        assert_eq!(classify_command("git checkout main"), CommandSafety::Unsafe);
        assert_eq!(classify_command("git rebase main"), CommandSafety::Unsafe);
        assert_eq!(classify_command("git merge feature"), CommandSafety::Unsafe);
        assert_eq!(classify_command("git stash pop"), CommandSafety::Unsafe);
        assert_eq!(classify_command("git stash drop"), CommandSafety::Unsafe);
        assert_eq!(classify_command("git clean -fd"), CommandSafety::Unsafe);
    }

    // -- Unsafe cargo commands --

    #[test]
    fn test_unsafe_cargo_commands() {
        assert_eq!(classify_command("cargo build"), CommandSafety::Unsafe);
        assert_eq!(classify_command("cargo run"), CommandSafety::Unsafe);
        assert_eq!(classify_command("cargo test"), CommandSafety::Unsafe);
        assert_eq!(
            classify_command("cargo install crate"),
            CommandSafety::Unsafe
        );
        assert_eq!(classify_command("cargo clean"), CommandSafety::Unsafe);
        assert_eq!(classify_command("cargo publish"), CommandSafety::Unsafe);
    }

    // -- Unsafe package managers --

    #[test]
    fn test_unsafe_package_managers() {
        assert_eq!(
            classify_command("pip install requests"),
            CommandSafety::Unsafe
        );
        assert_eq!(
            classify_command("npm install -g typescript"),
            CommandSafety::Unsafe
        );
        assert_eq!(
            classify_command("npm install --global eslint"),
            CommandSafety::Unsafe
        );
        assert_eq!(
            classify_command("npm install express"),
            CommandSafety::Unsafe
        );
    }

    // -- Unsafe docker commands --

    #[test]
    fn test_unsafe_docker_commands() {
        assert_eq!(
            classify_command("docker rm container"),
            CommandSafety::Unsafe
        );
        assert_eq!(classify_command("docker rmi image"), CommandSafety::Unsafe);
        assert_eq!(
            classify_command("docker stop container"),
            CommandSafety::Unsafe
        );
        assert_eq!(
            classify_command("docker kill container"),
            CommandSafety::Unsafe
        );
        assert_eq!(classify_command("docker prune"), CommandSafety::Unsafe);
    }

    #[test]
    fn test_safe_docker_commands() {
        assert_eq!(classify_command("docker ps"), CommandSafety::Safe);
        assert_eq!(classify_command("docker images"), CommandSafety::Safe);
        assert_eq!(
            classify_command("docker logs container"),
            CommandSafety::Safe
        );
        assert_eq!(
            classify_command("docker inspect container"),
            CommandSafety::Safe
        );
        assert_eq!(classify_command("docker version"), CommandSafety::Safe);
    }

    // -- Redirect tests --

    #[test]
    fn test_redirect_is_unsafe() {
        assert_eq!(
            classify_command("echo hello > file.txt"),
            CommandSafety::Unsafe
        );
        assert_eq!(classify_command("ls >> output.log"), CommandSafety::Unsafe);
        // Even safe commands become unsafe with redirect.
        assert_eq!(classify_command("cat file > copy"), CommandSafety::Unsafe);
    }

    // -- Pipe chain tests --

    #[test]
    fn test_pipe_safe_safe() {
        assert_eq!(classify_command("ls | wc -l"), CommandSafety::Safe);
        assert_eq!(
            classify_command("cat file.txt | grep pattern"),
            CommandSafety::Safe
        );
    }

    #[test]
    fn test_pipe_safe_unsafe() {
        // Pipe containing an unsafe command makes the whole chain unsafe.
        assert_eq!(classify_command("ls | rm"), CommandSafety::Unsafe);
    }

    // -- Ambiguous command tests --

    #[test]
    fn test_ambiguous_commands() {
        assert_eq!(classify_command("vim file.txt"), CommandSafety::Ambiguous);
        assert_eq!(classify_command("node script.js"), CommandSafety::Ambiguous);
        assert_eq!(classify_command("python app.py"), CommandSafety::Ambiguous);
        assert_eq!(
            classify_command("curl http://example.com"),
            CommandSafety::Ambiguous
        );
        assert_eq!(classify_command("docker build ."), CommandSafety::Ambiguous);
    }

    #[test]
    fn test_ambiguous_empty() {
        assert_eq!(classify_command(""), CommandSafety::Ambiguous);
        assert_eq!(classify_command("   "), CommandSafety::Ambiguous);
    }

    // -- Edge cases --

    #[test]
    fn test_path_prefix() {
        assert_eq!(classify_command("/usr/bin/ls"), CommandSafety::Safe);
        assert_eq!(classify_command("/bin/rm file"), CommandSafety::Unsafe);
        assert_eq!(
            classify_command("/usr/local/bin/git status"),
            CommandSafety::Safe
        );
    }

    #[test]
    fn test_leading_whitespace() {
        assert_eq!(classify_command("  ls -la"), CommandSafety::Safe);
        assert_eq!(classify_command("  rm file"), CommandSafety::Unsafe);
    }

    #[test]
    fn test_bare_commands() {
        // Bare git/cargo with no subcommand are ambiguous.
        assert_eq!(classify_command("git"), CommandSafety::Ambiguous);
        assert_eq!(classify_command("cargo"), CommandSafety::Ambiguous);
    }
}
