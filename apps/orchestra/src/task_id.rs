/// Newtype wrapper for task IDs providing consistent branch names, worktree
/// directory names, and log prefixes.
///
/// All call sites that need a short prefix, a branch name, or a worktree
/// directory name should use this type instead of slicing or formatting
/// manually.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaskId(String);

impl TaskId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Full task ID string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 12-character short prefix used in branch names and log messages.
    pub fn short(&self) -> &str {
        &self.0[..12.min(self.0.len())]
    }

    /// Git branch name: `agent/task-{short}`.
    pub fn branch_name(&self) -> String {
        format!("agent/task-{}", self.short())
    }

    /// Worktree directory name: `task-{short}`.
    pub fn worktree_dir_name(&self) -> String {
        format!("task-{}", self.short())
    }
}

/// Displays the short 12-character prefix, suitable for log messages.
impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.short())
    }
}

impl AsRef<str> for TaskId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<String> for TaskId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for TaskId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}
