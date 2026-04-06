//! Doom Loop Detection
//!
//! Detects when an agent gets stuck in repetitive patterns of tool calls,
//! preventing infinite loops and providing helpful suggestions to break out.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

const DEFAULT_WINDOW_SIZE: usize = 10;
const DEFAULT_THRESHOLD: usize = 3;
const LOOP_TIME_WINDOW: Duration = Duration::from_secs(300);

#[derive(Debug, Clone, Eq)]
pub struct ToolCallPattern {
    pub tool_name: String,
    pub input_hash: u64,
    pub timestamp: Instant,
}

impl PartialEq for ToolCallPattern {
    fn eq(&self, other: &Self) -> bool {
        self.tool_name == other.tool_name && self.input_hash == other.input_hash
    }
}

impl Hash for ToolCallPattern {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.tool_name.hash(state);
        self.input_hash.hash(state);
    }
}

impl ToolCallPattern {
    pub fn new(tool_name: &str, input: &Value) -> Self {
        Self {
            tool_name: tool_name.to_string(),
            input_hash: Self::hash_input(input),
            timestamp: Instant::now(),
        }
    }

    fn hash_input(input: &Value) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();

        match input {
            Value::Object(map) => {
                let mut sorted: Vec<_> = map.iter().collect();
                sorted.sort_by(|a, b| a.0.cmp(b.0));
                for (k, v) in sorted {
                    k.hash(&mut hasher);
                    Self::hash_value(v, &mut hasher);
                }
            }
            Value::Array(arr) => {
                for v in arr {
                    Self::hash_value(v, &mut hasher);
                }
            }
            _ => {
                input.hash(&mut hasher);
            }
        }

        hasher.finish()
    }

    fn hash_value(value: &Value, hasher: &mut impl Hasher) {
        match value {
            Value::Null => 0u8.hash(hasher),
            Value::Bool(b) => b.hash(hasher),
            Value::Number(n) => n.to_string().hash(hasher),
            Value::String(s) => s.hash(hasher),
            Value::Array(arr) => {
                for v in arr {
                    Self::hash_value(v, hasher);
                }
            }
            Value::Object(map) => {
                let mut sorted: Vec<_> = map.iter().collect();
                sorted.sort_by(|a, b| a.0.cmp(b.0));
                for (k, v) in sorted {
                    k.hash(hasher);
                    Self::hash_value(v, hasher);
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoomLoopWarning {
    pub tool: String,
    pub repeats: usize,
    pub suggestion: String,
    pub patterns: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct LoopStatistics {
    pub total_tool_calls: usize,
    pub unique_patterns: usize,
    pub loop_warnings: usize,
}

#[derive(Debug)]
pub struct DoomLoopDetector {
    history: VecDeque<ToolCallPattern>,
    threshold: usize,
    window_size: usize,
    stats: LoopStatistics,
    last_warning: Option<Instant>,
    min_warning_interval: Duration,
}

impl Default for DoomLoopDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl DoomLoopDetector {
    pub fn new() -> Self {
        Self {
            history: VecDeque::with_capacity(DEFAULT_WINDOW_SIZE),
            threshold: DEFAULT_THRESHOLD,
            window_size: DEFAULT_WINDOW_SIZE,
            stats: LoopStatistics::default(),
            last_warning: None,
            min_warning_interval: Duration::from_secs(30),
        }
    }

    pub fn with_config(threshold: usize, window_size: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(window_size),
            threshold,
            window_size,
            stats: LoopStatistics::default(),
            last_warning: None,
            min_warning_interval: Duration::from_secs(30),
        }
    }

    pub fn with_warning_interval(mut self, interval: Duration) -> Self {
        self.min_warning_interval = interval;
        self
    }

    pub fn record(&mut self, tool_name: &str, input: &Value) -> Option<DoomLoopWarning> {
        let pattern = ToolCallPattern::new(tool_name, input);

        self.prune_old_patterns();

        self.history.push_back(pattern.clone());

        while self.history.len() > self.window_size {
            self.history.pop_front();
        }

        self.stats.total_tool_calls += 1;
        self.stats.unique_patterns = self.count_unique_patterns();

        let warning = self.check_for_loop(&pattern);

        if warning.is_some() {
            self.stats.loop_warnings += 1;
            self.last_warning = Some(Instant::now());
        }

        warning
    }

    fn prune_old_patterns(&mut self) {
        let cutoff = Instant::now() - LOOP_TIME_WINDOW;
        while let Some(oldest) = self.history.front() {
            if oldest.timestamp < cutoff {
                self.history.pop_front();
            } else {
                break;
            }
        }
    }

    fn count_unique_patterns(&self) -> usize {
        let mut unique = HashMap::new();
        for pattern in &self.history {
            unique.insert((pattern.tool_name.clone(), pattern.input_hash), true);
        }
        unique.len()
    }

    fn check_for_loop(&self, current: &ToolCallPattern) -> Option<DoomLoopWarning> {
        let count = self.history.iter().filter(|p| *p == current).count();

        if count >= self.threshold {
            if let Some(last) = self.last_warning {
                if last.elapsed() < self.min_warning_interval {
                    return None;
                }
            }

            Some(self.generate_warning(current, count))
        } else {
            None
        }
    }

    fn generate_warning(&self, pattern: &ToolCallPattern, count: usize) -> DoomLoopWarning {
        let suggestions = self.get_suggestions(&pattern.tool_name);

        DoomLoopWarning {
            tool: pattern.tool_name.clone(),
            repeats: count,
            suggestion: suggestions,
            patterns: self.get_recent_patterns(),
        }
    }

    fn get_suggestions(&self, tool_name: &str) -> String {
        match tool_name {
            "Read" => {
                "The agent is repeatedly reading the same files. Consider using Glob or Grep to find patterns, or the agent should stop and analyze what it's found.".to_string()
            }
            "Bash" => {
                "The agent is repeatedly running the same commands. Check if the command is producing expected output, or if there's a logic error causing repeated execution.".to_string()
            }
            "Edit" | "Write" => {
                "The agent is repeatedly editing the same content. This might indicate a stuck loop - the edit might not be taking effect, or the agent isn't checking results.".to_string()
            }
            "Glob" | "Grep" => {
                "The agent is repeatedly searching for files. This might indicate confusion about the codebase structure, or the search parameters need adjustment.".to_string()
            }
            "WebFetch" => {
                "The agent is repeatedly fetching web content. Check if the URLs are correct, or if the agent is stuck in a scraping loop.".to_string()
            }
            _ => {
                format!(
                    "The agent is calling this tool repeatedly. This pattern suggests the agent is stuck in a loop and not making progress. Consider interrupting with Ctrl+C."
                )
            }
        }
    }

    fn get_recent_patterns(&self) -> Vec<String> {
        self.history
            .iter()
            .rev()
            .take(5)
            .map(|p| format!("{}({})", p.tool_name, p.input_hash % 1000))
            .collect()
    }

    pub fn statistics(&self) -> &LoopStatistics {
        &self.stats
    }

    pub fn reset(&mut self) {
        self.history.clear();
        self.stats = LoopStatistics::default();
        self.last_warning = None;
    }

    pub fn history_size(&self) -> usize {
        self.history.len()
    }
}
