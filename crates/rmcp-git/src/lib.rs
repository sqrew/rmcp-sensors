use git2::{Repository, StatusOptions};
use rmcp::{
    handler::server::{router::tool::ToolRouter, ServerHandler, wrapper::Parameters},
    model::*,
    ErrorData as McpError,
};
use schemars::JsonSchema;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RepoPathParams {
    #[schemars(description = "Path to the git repository (defaults to current directory)")]
    pub path: Option<String>,
}

#[derive(Debug)]
pub struct GitServer {
    pub tool_router: ToolRouter<Self>,
}

impl Default for GitServer {
    fn default() -> Self {
        Self::new()
    }
}

impl GitServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    fn get_repo(path: Option<String>) -> Result<Repository, McpError> {
        let repo_path = path
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        Repository::discover(&repo_path)
            .map_err(|e| McpError::internal_error(format!("Not a git repository: {}", e), None))
    }
}

#[rmcp::tool_router]
impl GitServer {
    #[rmcp::tool(description = "Get git repository status (branch, uncommitted changes, last commit)")]
    pub async fn get_status(
        &self,
        Parameters(params): Parameters<RepoPathParams>,
    ) -> Result<CallToolResult, McpError> {
        let repo = Self::get_repo(params.path)?;
        let mut result = String::from("Git Repository Status:\n\n");

        // Repository path
        if let Some(workdir) = repo.workdir() {
            result.push_str(&format!("Repository: {}\n", workdir.display()));
        }

        // Current branch
        match repo.head() {
            Ok(head) => {
                if let Some(name) = head.shorthand() {
                    result.push_str(&format!("Branch: {}\n", name));
                }

                // Last commit
                if let Ok(commit) = head.peel_to_commit() {
                    let id = commit.id();
                    let short_id = &id.to_string()[..7];
                    let summary = commit.summary().unwrap_or("(no message)");
                    let time = commit.time();
                    let timestamp = chrono::DateTime::from_timestamp(time.seconds(), 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                        .unwrap_or_else(|| "unknown".to_string());

                    result.push_str(&format!("\nLast Commit:\n"));
                    result.push_str(&format!("  {} - {}\n", short_id, summary));
                    result.push_str(&format!("  Author: {}\n", commit.author().name().unwrap_or("unknown")));
                    result.push_str(&format!("  Date: {}\n", timestamp));
                }
            }
            Err(_) => {
                result.push_str("Branch: (no commits yet)\n");
            }
        }

        // Status - uncommitted changes
        let mut opts = StatusOptions::new();
        opts.include_untracked(true);
        opts.recurse_untracked_dirs(true);

        match repo.statuses(Some(&mut opts)) {
            Ok(statuses) => {
                let mut staged = Vec::new();
                let mut modified = Vec::new();
                let mut untracked = Vec::new();

                for entry in statuses.iter() {
                    let path = entry.path().unwrap_or("?");
                    let status = entry.status();

                    if status.is_index_new() || status.is_index_modified() || status.is_index_deleted() {
                        staged.push(path.to_string());
                    }
                    if status.is_wt_modified() || status.is_wt_deleted() {
                        modified.push(path.to_string());
                    }
                    if status.is_wt_new() {
                        untracked.push(path.to_string());
                    }
                }

                result.push_str("\nWorking Tree:\n");

                if staged.is_empty() && modified.is_empty() && untracked.is_empty() {
                    result.push_str("  Clean - nothing to commit\n");
                } else {
                    if !staged.is_empty() {
                        result.push_str(&format!("  Staged: {} file(s)\n", staged.len()));
                        for f in staged.iter().take(5) {
                            result.push_str(&format!("    + {}\n", f));
                        }
                        if staged.len() > 5 {
                            result.push_str(&format!("    ... and {} more\n", staged.len() - 5));
                        }
                    }
                    if !modified.is_empty() {
                        result.push_str(&format!("  Modified: {} file(s)\n", modified.len()));
                        for f in modified.iter().take(5) {
                            result.push_str(&format!("    M {}\n", f));
                        }
                        if modified.len() > 5 {
                            result.push_str(&format!("    ... and {} more\n", modified.len() - 5));
                        }
                    }
                    if !untracked.is_empty() {
                        result.push_str(&format!("  Untracked: {} file(s)\n", untracked.len()));
                        for f in untracked.iter().take(5) {
                            result.push_str(&format!("    ? {}\n", f));
                        }
                        if untracked.len() > 5 {
                            result.push_str(&format!("    ... and {} more\n", untracked.len() - 5));
                        }
                    }
                }
            }
            Err(e) => {
                result.push_str(&format!("\nCould not get status: {}\n", e));
            }
        }

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[rmcp::tool(description = "Get recent git commits (last 10)")]
    pub async fn get_log(
        &self,
        Parameters(params): Parameters<RepoPathParams>,
    ) -> Result<CallToolResult, McpError> {
        let repo = Self::get_repo(params.path)?;
        let mut result = String::from("Recent Commits:\n\n");

        let head = repo.head()
            .map_err(|e| McpError::internal_error(format!("No HEAD: {}", e), None))?;

        let oid = head.target()
            .ok_or_else(|| McpError::internal_error("HEAD has no target", None))?;

        let mut revwalk = repo.revwalk()
            .map_err(|e| McpError::internal_error(format!("Failed to create revwalk: {}", e), None))?;

        revwalk.push(oid)
            .map_err(|e| McpError::internal_error(format!("Failed to push HEAD: {}", e), None))?;

        let mut count = 0;
        for oid in revwalk.take(10) {
            if let Ok(oid) = oid {
                if let Ok(commit) = repo.find_commit(oid) {
                    count += 1;
                    let id_str = oid.to_string();
                    let short_id = &id_str[..7];
                    let summary = commit.summary().unwrap_or("(no message)").to_string();
                    let author = commit.author();
                    let author_name = author.name().unwrap_or("unknown");

                    result.push_str(&format!("{} {} - {}\n", short_id, author_name, summary));
                }
            }
        }

        if count == 0 {
            result.push_str("No commits found.\n");
        }

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }
}

#[rmcp::tool_handler]
impl ServerHandler for GitServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some("Cross-platform Git repository information server".into()),
        }
    }
}
