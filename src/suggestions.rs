use std::{
    collections::HashSet,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

#[derive(Clone, Debug, PartialEq)]
pub struct Suggestion {
    pub command: String,
    pub detail: String,
}

const COMMANDS: &[(&str, &str)] = &[
    ("git status", "Inspect working tree changes"),
    ("git status --short", "Compact working tree status"),
    ("git diff", "Review unstaged changes"),
    ("git diff --staged", "Review staged changes"),
    ("git log --oneline -12", "Recent commits"),
    ("git branch", "List local branches"),
    ("git switch ", "Switch to a branch · Tab to complete"),
    ("git add ", "Stage files · Tab to complete"),
    ("git commit -m ", "Commit staged changes"),
    ("git pull --ff-only", "Pull without a merge commit"),
    ("git push", "Push current branch"),
    ("cargo run", "Build and run the Rust project"),
    ("cargo test", "Run Rust tests"),
    ("cargo clippy", "Check Rust code with Clippy"),
    ("cargo build --release", "Build an optimized executable"),
    ("ls -lah", "List files with sizes and hidden entries"),
    ("cd ", "Change directory · Tab to complete paths"),
    ("pwd", "Print working directory"),
    ("clear", "Clear the terminal viewport"),
    ("npm run dev", "Start the development server"),
    ("npm test", "Run project tests"),
    ("gh pr list", "List pull requests"),
];

#[derive(Default)]
pub struct Suggestions {
    history: Vec<String>,
}

impl Suggestions {
    pub fn load_history(&mut self, path: &Path) {
        let Ok(mut file) = std::fs::File::open(path) else {
            return;
        };
        let length = file.metadata().map(|m| m.len()).unwrap_or(0);
        let start = length.saturating_sub(256 * 1024);
        if file.seek(SeekFrom::Start(start)).is_err() {
            return;
        }
        let mut bytes = Vec::new();
        if file.take(256 * 1024).read_to_end(&mut bytes).is_err() {
            return;
        }
        let text = String::from_utf8_lossy(&bytes);
        self.history = text
            .lines()
            .skip(usize::from(start > 0))
            .filter(|line| !line.starts_with(['#', ' ']) && !line.chars().any(char::is_control))
            .map(str::to_owned)
            .collect();
    }

    pub fn recent(&self, limit: usize) -> &[String] {
        let start = self.history.len().saturating_sub(limit);
        &self.history[start..]
    }

    pub fn matching(&self, query: &str) -> Vec<Suggestion> {
        if query.is_empty() || query.starts_with(' ') || query.chars().any(char::is_control) {
            return vec![];
        }
        let mut seen = HashSet::new();
        let mut result = Vec::new();
        for (command, detail) in self
            .history
            .iter()
            .rev()
            .map(|h| (h.as_str(), "Bash history"))
            .chain(COMMANDS.iter().copied())
        {
            if command.starts_with(query) && command != query && seen.insert(command.to_owned()) {
                result.push(Suggestion {
                    command: command.into(),
                    detail: detail.into(),
                });
                if result.len() == 4 {
                    break;
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ranks_recent_history_and_deduplicates() {
        let suggestions = Suggestions {
            history: vec!["git status".into(), "git diff".into(), "git status".into()],
        };
        let result = suggestions.matching("git ");
        assert_eq!(result[0].command, "git status");
        assert_eq!(result[1].command, "git diff");
        assert_eq!(
            result.iter().filter(|s| s.command == "git status").count(),
            1
        );
        assert!(suggestions.matching(" password").is_empty());
    }
}
