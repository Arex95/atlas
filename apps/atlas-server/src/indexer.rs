use crate::constants::{errors, indexer as idx_consts};
use std::path::PathBuf;
use tracing::{error, info};
use walkdir::WalkDir;

pub struct ProjectIndexer {
    root_path: PathBuf,
}

impl ProjectIndexer {
    pub fn new(root_path: &str) -> Self {
        Self {
            root_path: PathBuf::from(root_path),
        }
    }

    pub async fn run(&self) -> Result<String, String> {
        info!(
            "[Indexer] Starting index for project at {:?}",
            self.root_path
        );

        if !self.root_path.exists() {
            return Err(errors::PROJECT_PATH_NOT_EXIST.to_string());
        }

        let mut index_content = String::new();
        index_content.push_str(idx_consts::HEADER);
        index_content.push_str(idx_consts::STRUCTURE_HEADER);

        for entry in WalkDir::new(&self.root_path)
            .into_iter()
            .filter_entry(|e| !Self::is_ignored(e))
        {
            let entry = entry.map_err(|e| e.to_string())?;
            let depth = entry.depth();
            let name = entry.file_name().to_string_lossy();
            let _path = entry
                .path()
                .strip_prefix(&self.root_path)
                .unwrap_or(entry.path());

            if entry.file_type().is_dir() {
                if depth > 0 {
                    index_content.push_str(&format!("{}📂 **{}/**\n", "  ".repeat(depth), name));
                }
            } else {
                index_content.push_str(&format!("{}📄 {}\n", "  ".repeat(depth), name));
            }
        }

        let index_path = self.root_path.join("PROJECT_INDEX.md");
        match tokio::fs::write(&index_path, &index_content).await {
            Ok(()) => {
                info!(
                    "[Indexer] Project index generated successfully at {:?}",
                    index_path
                );
                Ok(index_content)
            }
            Err(e) => {
                error!("[Indexer] Failed to write PROJECT_INDEX.md: {}", e);
                Err(format!("Failed to write index file: {}", e))
            }
        }
    }

    fn is_ignored(entry: &walkdir::DirEntry) -> bool {
        let name = entry.file_name().to_string_lossy();
        idx_consts::IGNORED.iter().any(|&ignored| name == ignored)
    }
}
