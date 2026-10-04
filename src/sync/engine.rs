use crate::vfs::db::MetadataDb;
use crate::vfs::node::Node;
use crate::provider::telegram::TelegramProvider;

#[derive(Debug)]
pub enum SyncEvent {
    /// A file exists locally but is not on the remote (needs upload)
    NeedsUpload { node: Node },
    /// A file is on the remote but not cached locally (or changed remotely)
    NeedsDownload { node: Node },
}

pub struct SyncEngine {
    telegram: TelegramProvider,
}

impl SyncEngine {
    pub fn new(telegram: TelegramProvider) -> Self {
        Self { telegram }
    }

    pub fn telegram(&self) -> &TelegramProvider {
        &self.telegram
    }

    /// Scans the metadata database for files that have no corresponding StorageObject.
    pub fn scan_for_uploads(&self, db: &MetadataDb) -> Result<Vec<SyncEvent>, String> {
        let unsynced_nodes = db.get_unsynced_files().map_err(|e| e.to_string())?;
        
        let mut events = Vec::new();
        for node in unsynced_nodes {
            events.push(SyncEvent::NeedsUpload { node });
        }
        
        Ok(events)
    }

    /// Uploads the file to Telegram and returns the remote ID.
    pub async fn process_upload(&self, node: &Node) -> Result<String, String> {
        println!("  [SyncEngine] Uploading file '{}' to Telegram...", node.name);
        
        // Read the actual file bytes from the local disk cache
        let cache_path = format!(".mycloud_cache/{}", node.id);
        let file_data = tokio::fs::read(&cache_path).await.map_err(|e| {
            format!("Failed to read local cache for {}: {}", node.name, e)
        })?;
        
        let remote_id = self.telegram.upload_bytes(&node.name, file_data).await?;
        
        println!("  [SyncEngine] Upload complete! Telegram File ID: {}", remote_id);
        Ok(remote_id)
    }

    pub async fn download_to_file(&self, remote_id: &str, dest_path: &str) -> Result<(), String> {
        self.telegram.download_to_file(remote_id, dest_path).await
    }
}
