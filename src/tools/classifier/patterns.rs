//! Bash Safety Pattern Definitions
//!
//! Regex patterns for dangerous command detection.

use regex::Regex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafetyLevel {
    Safe,
    Dangerous,
    Critical,
}

#[derive(Debug, Clone)]
pub struct SafetyPattern {
    pub level: SafetyLevel,
    pub pattern: Regex,
    pub message: &'static str,
}

lazy_static::lazy_static! {
    pub static ref SAFETY_PATTERNS: Vec<SafetyPattern> = vec![
        // CRITICAL: Data destruction
        SafetyPattern {
            level: SafetyLevel::Critical,
            pattern: Regex::new(r"(?i)\b(rm\s+-rf\s+/|rm\s+-rf\s+\*\s*$|mkfs\.|dd\s+if=/dev/zero\s+)").unwrap(),
            message: "Critical: File system format or recursive deletion",
        },
        SafetyPattern {
            level: SafetyLevel::Critical,
            pattern: Regex::new(r"(?i)\b(sudo\s+rm\s+-rf|sudo\s+rm\s+-r\s+/|>\s*/dev/sd)").unwrap(),
            message: "Critical: Elevated destructive command",
        },
        SafetyPattern {
            level: SafetyLevel::Critical,
            pattern: Regex::new(r"(?i)\b(drop\s+(database|table)|delete\s+from\s+\w+\s*;)").unwrap(),
            message: "Critical: Database destruction",
        },
        // DANGEROUS: System modification
        SafetyPattern {
            level: SafetyLevel::Dangerous,
            pattern: Regex::new(r"(?i)\b(chmod\s+777|chmod\s+-R\s+777)").unwrap(),
            message: "Dangerous: Overly permissive file permissions",
        },
        SafetyPattern {
            level: SafetyLevel::Dangerous,
            pattern: Regex::new(r"(?i)\b(chown\s+-R\s+root|chgrp\s+root|sudo\s+chmod)").unwrap(),
            message: "Dangerous: Ownership changes",
        },
        SafetyPattern {
            level: SafetyLevel::Dangerous,
            pattern: Regex::new(r"(?i)\b(iptables|ufw|firewall-cmd)\s+(?!.*--list)").unwrap(),
            message: "Dangerous: Firewall modification",
        },
        SafetyPattern {
            level: SafetyLevel::Dangerous,
            pattern: Regex::new(r"(?i)\b(curl|wget).*\|.*(bash|sh|perl|python)").unwrap(),
            message: "Dangerous: Pipe to shell execution",
        },
        SafetyPattern {
            level: SafetyLevel::Dangerous,
            pattern: Regex::new(r"(?i)\b(kill\s+(-9|-SIGKILL)|pkill\s+-9)").unwrap(),
            message: "Dangerous: Force kill processes",
        },
        SafetyPattern {
            level: SafetyLevel::Dangerous,
            pattern: Regex::new(r"(?i)\b(systemctl\s+(stop|disable)|service\s+\w+\s+stop)").unwrap(),
            message: "Dangerous: Service management",
        },
        // SAFE patterns (no matches = safe)
    ];
}

pub fn check_command(command: &str) -> Option<(&'static str, SafetyLevel)> {
    for pattern in SAFETY_PATTERNS.iter() {
        if pattern.pattern.is_match(command) {
            return Some((pattern.message, pattern.level));
        }
    }
    None
}
