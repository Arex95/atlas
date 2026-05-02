pub mod env {
    pub const DATABASE_URL: &str = "DATABASE_URL";
    pub const WEB_ORIGIN: &str = "WEB_ORIGIN";
    pub const PORT: &str = "PORT";
    pub const ATLAS_MCP_TOKEN: &str = "ATLAS_MCP_TOKEN";
    pub const ATLAS_API_TOKEN: &str = "ATLAS_API_TOKEN";
    pub const ATLAS_SERVER_URL: &str = "ATLAS_SERVER_URL";
    pub const ATLAS_BIN_DIR: &str = "ATLAS_BIN_DIR";
    pub const ATLAS_DEFAULT_AUTHOR: &str = "ATLAS_DEFAULT_AUTHOR";
    pub const ATLAS_PROJECT_ID: &str = "ATLAS_PROJECT_ID";
    pub const ATLAS_SESSION_ID: &str = "ATLAS_SESSION_ID";
}

pub mod defaults {
    pub const PORT: u16 = 4000;
    pub const WEB_ORIGIN: &str = "http://localhost:3000";
    pub const SERVER_URL: &str = "http://localhost:4000";
    pub const AUTHOR: &str = "anon";
    pub const BIN_DIR: &str = "./bin";
}

pub mod errors {
    pub const SESSION_NOT_FOUND: &str = "Session not found";
    pub const PROJECT_NOT_FOUND: &str = "Project not found";
    pub const KEY_NOT_FOUND: &str = "Key not found";
    pub const INTERNAL: &str = "Internal server error";
    pub const PROJECT_CREATE_FAILED: &str = "Failed to create project";
    pub const SESSION_HISTORY_FAILED: &str = "Failed to load session history";
    pub const MESSAGE_CREATE_FAILED: &str = "Failed to create message";
    pub const READ_FILE_FAILED: &str = "Failed to read file";
    pub const PROJECT_PATH_NOT_EXIST: &str = "Project path does not exist";
    pub const PATH_NOT_DIRECTORY: &str = "Path is not a directory";
    pub const CANNOT_READ_DIRECTORY: &str = "Cannot read a directory";
    pub const FILE_TOO_LARGE: &str = "File exceeds 5MB limit";
    pub const MISSING_PARAMS: &str = "Missing required parameters";
    pub const PATH_DOES_NOT_EXIST: &str = "Path does not exist";
    pub const PATH_OUTSIDE_ROOT: &str = "Path is outside any registered project root";
    pub const DOCUMENT_NOT_FOUND: &str = "Document not found";
    pub const SKILL_NOT_FOUND: &str = "Skill not found";
    pub const NOTIFICATION_NOT_FOUND: &str = "Notification not found";
    pub const REMINDER_NOT_FOUND: &str = "Reminder not found";
    pub const PROMPT_NOT_FOUND: &str = "Prompt not found";
}

pub mod response {
    pub const STATUS_SUCCESS: &str = "success";
    pub const STATUS_ERROR: &str = "error";
    pub const PROJECT_INDEXED: &str = "Project indexed successfully";
    pub const MESSAGE_SENT: &str = "Message sent successfully";
}

pub mod terminal {
    pub const TERM_VAR: &str = "TERM";
    pub const TERM_TYPE: &str = "xterm-256color";
    pub const MCP_AGENT_ID: &str = "MCP_AGENT";
    pub const WELCOME: &str =
        "\r\n\x1b[1;35m[ATLAS ORCHESTRATOR]\x1b[0m Welcome to the synchronized terminal.\r\n";
    pub const WELCOME_HINT: &str =
        "Type \x1b[1;33matlas list\x1b[0m to discover other active agents.\r\n\r\n";
    pub const DANGEROUS_COMMANDS: &[&str] =
        &["rm", "mv", "dd", "chmod", "chown", "mkfs", "sudo", "rmdir"];
}

pub mod indexer {
    pub const HEADER: &str = "# Project Index: Atlas Orchestrator\n\n";
    pub const STRUCTURE_HEADER: &str = "## Structure\n\n";
    pub const IGNORED: &[&str] =
        &["node_modules", ".git", "target", "dist", ".next", "PROJECT_INDEX.md"];
}
