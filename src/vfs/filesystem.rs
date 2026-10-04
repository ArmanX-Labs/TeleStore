use chrono::Utc;
use super::node::{Node, NodeId, NodeType};
use super::db::MetadataDb;
use uuid::Uuid;

pub struct FileSystem {
    db: MetadataDb,
    root_id: NodeId,
}

impl FileSystem {
    pub fn new() -> Self {
        Self::new_in_memory().unwrap()
    }

    pub fn new_in_memory() -> Result<Self, String> {
        let db = MetadataDb::new_in_memory().map_err(|e| e.to_string())?;
        Self::init_with_db(db)
    }

    pub fn new_with_db(db_path: &str) -> Result<Self, String> {
        let db = MetadataDb::new(db_path).map_err(|e| e.to_string())?;
        Self::init_with_db(db)
    }

    fn init_with_db(db: MetadataDb) -> Result<Self, String> {
        let root_node_opt = db.get_node_by_parent_and_name(None, "/").map_err(|e| e.to_string())?;
        
        let root_id = if let Some(root_node) = root_node_opt {
            root_node.id
        } else {
            let now = Utc::now();
            let root = Node {
                id: Uuid::new_v4(),
                parent_id: None,
                name: "/".to_string(),
                node_type: NodeType::Directory,
                size: 0,
                hash: "".to_string(),
                version: 1,
                created_at: now,
                modified_at: now,
            };
            let id = root.id;
            db.insert_node(&root).map_err(|e| e.to_string())?;
            id
        };

        Ok(Self { db, root_id })
    }

    pub fn db(&self) -> &MetadataDb {
        &self.db
    }

    pub fn mkdir(&mut self, path: &str) -> Result<NodeId, String> {

        let path = path.trim_matches('/');

        if path.is_empty(){
            return Err("Can't create Root Directory".to_string());
        }

        let mut components = path.rsplitn(2, '/');

        let name = components.next().unwrap();

        let parent_path = components.next().unwrap_or("");

        let parent_id = self.resolve_path(parent_path)?;

        let parent = self
            .db
            .get_node(&parent_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Parent directory is not found".to_string())?;

        if parent.node_type != NodeType::Directory{
            return Err("Parent is not a Directory".to_string())
        }
           // prevent duplicate
        let existing = self.db.get_node_by_parent_and_name(Some(parent_id), name).map_err(|e| e.to_string())?;
        if existing.is_some() {
            return Err(format!("Already exists: {path}"))
        }

        let now = Utc::now();
        let node = Node {
            id: Uuid::new_v4(),
            parent_id: Some(parent_id),
            name: name.to_string(),
            node_type: NodeType::Directory,
            size: 0,
            hash: "".to_string(),
            version: 1,
            created_at: now,
            modified_at: now,
        };

        let node_id = node.id;

        self.db.insert_node(&node).map_err(|e| e.to_string())?;
        

       Ok(node_id)
    }

    pub fn create_file(&mut self, path: &str) -> Result<NodeId, String> {
        let path = path.trim_matches('/');

        if path.is_empty(){
            return Err("Can't create Root Directory as a file".to_string());
        }

        let mut components = path.rsplitn(2, '/');
        let name = components.next().unwrap();
        let parent_path = components.next().unwrap_or("");

        let parent_id = self.resolve_path(parent_path)?;

        let parent = self
            .db
            .get_node(&parent_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Parent directory is not found".to_string())?;

        if parent.node_type != NodeType::Directory{
            return Err("Parent is not a directory".to_string())
        }
        
        // prevent duplicate
        let existing = self.db.get_node_by_parent_and_name(Some(parent_id), name).map_err(|e| e.to_string())?;
        if existing.is_some() {
            return Err(format!("Already exists: {path}"))
        }

        let now = Utc::now();
        let node = Node {
            id: Uuid::new_v4(),
            parent_id: Some(parent_id),
            name: name.to_string(),
            node_type: NodeType::File,
            size: 0,
            hash: "".to_string(),
            version: 1,
            created_at: now,
            modified_at: now,
        };

        let node_id = node.id;
        self.db.insert_node(&node).map_err(|e| e.to_string())?;
        
        Ok(node_id)
    }
    
    pub fn stat(&self, path: &str) -> Result<Node, String> {
        let node_id = self.resolve_path(path)?;
        self.db.get_node(&node_id).map_err(|e| e.to_string())?.ok_or_else(|| "Node not found".to_string())
    }

    pub fn list(&self, path: &str) -> Result<Vec<Node>, String> {
        let node_id = self.resolve_path(path)?;
        
        let node = self.db.get_node(&node_id).map_err(|e| e.to_string())?.ok_or_else(|| "Node not found".to_string())?;
        if node.node_type != NodeType::Directory {
            return Err("Not a directory".to_string());
        }

        self.db.get_children(&node_id).map_err(|e| e.to_string())
    }

    pub fn delete(&mut self, path: &str) -> Result<(), String> {
        let node_id = self.resolve_path(path)?;
        
        if node_id == self.root_id {
            return Err("Cannot delete root directory".to_string());
        }

        // Check if directory is empty
        let node = self.db.get_node(&node_id).map_err(|e| e.to_string())?.ok_or_else(|| "Node not found".to_string())?;
        if node.node_type == NodeType::Directory {
            let children = self.db.get_children(&node_id).map_err(|e| e.to_string())?;
            if !children.is_empty() {
                return Err("Directory is not empty".to_string());
            }
        }

        self.db.delete_node(&node_id).map_err(|e| e.to_string())?;
        Ok(())
    }
    
    pub fn rename(&mut self, path: &str, new_name: &str) -> Result<(), String> {
        let node_id = self.resolve_path(path)?;
        
        if node_id == self.root_id {
            return Err("Cannot rename root directory".to_string());
        }

        let mut node = self.db.get_node(&node_id).map_err(|e| e.to_string())?.unwrap();
        
        // Prevent duplicate names in the same parent directory
        let existing = self.db.get_node_by_parent_and_name(node.parent_id, new_name).map_err(|e| e.to_string())?;
        if existing.is_some() {
            return Err(format!("A file or directory named '{}' already exists here", new_name));
        }

        node.name = new_name.to_string();
        node.modified_at = Utc::now();
        
        self.db.update_node(&node).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn move_node(&mut self, source_path: &str, dest_parent_path: &str) -> Result<(), String> {
        let source_id = self.resolve_path(source_path)?;
        
        if source_id == self.root_id {
            return Err("Cannot move root directory".to_string());
        }
        
        let dest_parent_id = self.resolve_path(dest_parent_path)?;
        
        let dest_parent = self.db.get_node(&dest_parent_id).map_err(|e| e.to_string())?.ok_or_else(|| "Destination parent not found".to_string())?;
        if dest_parent.node_type != NodeType::Directory {
            return Err("Destination is not a directory".to_string());
        }

        let mut node = self.db.get_node(&source_id).map_err(|e| e.to_string())?.unwrap();

        // Prevent duplicate names in the destination directory
        let existing = self.db.get_node_by_parent_and_name(Some(dest_parent_id), &node.name).map_err(|e| e.to_string())?;
        if existing.is_some() {
            return Err(format!("A file or directory named '{}' already exists in the destination", node.name));
        }

        // Prevent moving a directory into itself or its children
        let mut current_parent = Some(dest_parent_id);
        while let Some(parent_id) = current_parent {
            if parent_id == source_id {
                return Err("Cannot move a directory into itself or its descendants".to_string());
            }
            if let Some(parent_node) = self.db.get_node(&parent_id).map_err(|e| e.to_string())? {
                current_parent = parent_node.parent_id;
            } else {
                break;
            }
        }

        node.parent_id = Some(dest_parent_id);
        node.modified_at = Utc::now();
        
        self.db.update_node(&node).map_err(|e| e.to_string())?;
        Ok(())
    }
    
    pub fn resolve_path(&self, path: &str) -> Result<NodeId, String> {
        let path = path.trim_matches('/');

        if path.is_empty(){
            return Ok(self.root_id);
        }

        let mut current_id = self.root_id;

        for component in path.split('/') {
            let child = self.db.get_node_by_parent_and_name(Some(current_id), component).map_err(|e| e.to_string())?;
            if let Some(node) = child {
                current_id = node.id;
            } else {
                return Err(format!("path not found: {path}"));
            }
        }
        Ok(current_id)
    }

    pub fn exists(&self, path: &str) -> bool {
        self.resolve_path(path).is_ok() 
    }

    pub fn write(&mut self, path: &str, data: &[u8]) -> Result<(), String> {
        use sha2::{Sha256, Digest};
        use std::fs;

        let node_id = self.resolve_path(path)?;
        let mut node = self.db.get_node(&node_id).map_err(|e| e.to_string())?.unwrap();
        
        if node.node_type != NodeType::File {
            return Err("Not a file".to_string());
        }

        // Write to local cache
        let cache_dir = ".mycloud_cache";
        fs::create_dir_all(cache_dir).map_err(|e| e.to_string())?;
        let cache_path = format!("{}/{}", cache_dir, node.id);
        fs::write(&cache_path, data).map_err(|e| e.to_string())?;

        // Update metadata
        node.size = data.len() as u64;
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        node.hash = result.iter().map(|b| format!("{:02x}", b)).collect::<String>();
        node.version += 1;
        node.modified_at = Utc::now();

        self.db.update_node(&node).map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn read(&self, path: &str) -> Result<Vec<u8>, String> {
        let node_id = self.resolve_path(path)?;
        
        let node = self.db.get_node(&node_id).map_err(|e| e.to_string())?.ok_or_else(|| "Node not found".to_string())?;
        if node.node_type != NodeType::File {
            return Err("Not a file".to_string());
        }

        // In a real system, we'd read from local cache or fetch from remote.
        // For the simulator, we just return empty bytes representing the "data".
        Ok(vec![0; node.size as usize])
    }
}