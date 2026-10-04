use rusqlite::{Connection, Result as RusqliteResult, params, OptionalExtension};
use super::node::{Node, NodeId, NodeType};

pub struct MetadataDb {
    conn: Connection,
}

impl MetadataDb {
    pub fn new(db_path: &str) -> RusqliteResult<Self> {
        let conn = Connection::open(db_path)?;
        let mut db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    pub fn new_in_memory() -> RusqliteResult<Self> {
        let conn = Connection::open_in_memory()?;
        let mut db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&mut self) -> RusqliteResult<()> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS nodes (
                id TEXT PRIMARY KEY,
                parent_id TEXT,
                name TEXT NOT NULL,
                node_type TEXT NOT NULL,
                size INTEGER NOT NULL,
                hash TEXT NOT NULL,
                version INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                modified_at TEXT NOT NULL
            )",
            [],
        )?;

        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS storage_objects (
                node_id TEXT PRIMARY KEY,
                provider TEXT NOT NULL,
                remote_id TEXT NOT NULL,
                size INTEGER NOT NULL,
                hash TEXT NOT NULL,
                FOREIGN KEY(node_id) REFERENCES nodes(id) ON DELETE CASCADE
            )",
            [],
        )?;

        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS local_objects (
                node_id TEXT,
                device_id TEXT NOT NULL,
                local_path TEXT NOT NULL,
                state TEXT NOT NULL,
                PRIMARY KEY (node_id, device_id),
                FOREIGN KEY(node_id) REFERENCES nodes(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Migration: Convert legacy BLOB UUIDs to TEXT strings
        let mut to_update = Vec::new();
        {
            let mut stmt = self.conn.prepare("SELECT rowid, id, parent_id FROM nodes WHERE typeof(id) = 'blob' OR typeof(parent_id) = 'blob'")?;
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                let rowid: i64 = row.get(0)?;
                
                let id_str = match row.get_ref(1)? {
                    rusqlite::types::ValueRef::Blob(b) => Some(uuid::Uuid::from_slice(b).unwrap().to_string()),
                    _ => None,
                };
                
                let parent_id_str = match row.get_ref(2)? {
                    rusqlite::types::ValueRef::Blob(b) => Some(uuid::Uuid::from_slice(b).unwrap().to_string()),
                    _ => None,
                };
                
                to_update.push((rowid, id_str, parent_id_str));
            }
        }

        if !to_update.is_empty() {
            self.conn.execute("PRAGMA foreign_keys = OFF", [])?;
            for (rowid, id_str, parent_id_str) in to_update {
                if let Some(id) = id_str {
                    // Update child tables if they had the blob
                    self.conn.execute("UPDATE storage_objects SET node_id = ?1 WHERE typeof(node_id) = 'blob' AND node_id = (SELECT id FROM nodes WHERE rowid = ?2)", params![id.clone(), rowid]).ok();
                    self.conn.execute("UPDATE local_objects SET node_id = ?1 WHERE typeof(node_id) = 'blob' AND node_id = (SELECT id FROM nodes WHERE rowid = ?2)", params![id.clone(), rowid]).ok();
                    self.conn.execute("UPDATE nodes SET id = ?1 WHERE rowid = ?2", params![id, rowid])?;
                }
                if let Some(pid) = parent_id_str {
                    self.conn.execute("UPDATE nodes SET parent_id = ?1 WHERE rowid = ?2", params![pid, rowid])?;
                }
            }
            self.conn.execute("PRAGMA foreign_keys = ON", [])?;
        }

        Ok(())
    }

    pub fn insert_node(&self, node: &Node) -> RusqliteResult<()> {
        let node_type_str = match node.node_type {
            NodeType::File => "File",
            NodeType::Directory => "Directory",
        };

        self.conn.execute(
            "INSERT INTO nodes (id, parent_id, name, node_type, size, hash, version, created_at, modified_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                node.id.to_string(),
                node.parent_id.map(|u| u.to_string()),
                node.name,
                node_type_str,
                node.size as i64,
                node.hash,
                node.version as i64,
                node.created_at.to_rfc3339(),
                node.modified_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn get_node(&self, id: &NodeId) -> RusqliteResult<Option<Node>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, parent_id, name, node_type, size, hash, version, created_at, modified_at FROM nodes WHERE id = ?1"
        )?;

        stmt.query_row(params![id.to_string()], |row| Self::map_node_row(row)).optional()
    }
    
    pub fn get_node_by_parent_and_name(&self, parent_id: Option<NodeId>, name: &str) -> RusqliteResult<Option<Node>> {
        if let Some(pid) = parent_id {
            let mut stmt = self.conn.prepare(
                "SELECT id, parent_id, name, node_type, size, hash, version, created_at, modified_at FROM nodes WHERE parent_id = ?1 AND name = ?2"
            )?;
            stmt.query_row(params![pid.to_string(), name], |row| Self::map_node_row(row)).optional()
        } else {
            let mut stmt = self.conn.prepare(
                "SELECT id, parent_id, name, node_type, size, hash, version, created_at, modified_at FROM nodes WHERE parent_id IS NULL AND name = ?1"
            )?;
            stmt.query_row(params![name], |row| Self::map_node_row(row)).optional()
        }
    }

    fn map_node_row(row: &rusqlite::Row) -> RusqliteResult<Node> {
        let node_type_str: String = row.get(3)?;
        let node_type = if node_type_str == "Directory" {
            NodeType::Directory
        } else {
            NodeType::File
        };

        let size_i64: i64 = row.get(4)?;
        let version_i64: i64 = row.get(6)?;

        let id_uuid = match row.get_ref(0)? {
            rusqlite::types::ValueRef::Text(t) => uuid::Uuid::parse_str(std::str::from_utf8(t).unwrap()).unwrap(),
            rusqlite::types::ValueRef::Blob(b) => uuid::Uuid::from_slice(b).unwrap(),
            _ => panic!("Invalid id type"),
        };

        let parent_id_uuid = match row.get_ref(1)? {
            rusqlite::types::ValueRef::Null => None,
            rusqlite::types::ValueRef::Text(t) => Some(uuid::Uuid::parse_str(std::str::from_utf8(t).unwrap()).unwrap()),
            rusqlite::types::ValueRef::Blob(b) => Some(uuid::Uuid::from_slice(b).unwrap()),
            _ => panic!("Invalid parent_id type"),
        };

        let created_at_str: String = row.get(7)?;
        let modified_at_str: String = row.get(8)?;

        Ok(Node {
            id: id_uuid,
            parent_id: parent_id_uuid,
            name: row.get(2)?,
            node_type,
            size: size_i64 as u64,
            hash: row.get(5)?,
            version: version_i64 as u64,
            created_at: chrono::DateTime::parse_from_rfc3339(&created_at_str).unwrap().with_timezone(&chrono::Utc),
            modified_at: chrono::DateTime::parse_from_rfc3339(&modified_at_str).unwrap().with_timezone(&chrono::Utc),
        })
    }

    pub fn get_children(&self, parent_id: &NodeId) -> RusqliteResult<Vec<Node>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, parent_id, name, node_type, size, hash, version, created_at, modified_at FROM nodes WHERE parent_id = ?1"
        )?;

        let node_iter = stmt.query_map(params![parent_id.to_string()], |row| Self::map_node_row(row))?;

        let mut children = Vec::new();
        for node in node_iter {
            children.push(node?);
        }
        Ok(children)
    }

    pub fn update_node(&self, node: &Node) -> RusqliteResult<()> {
        let node_type_str = match node.node_type {
            NodeType::File => "File",
            NodeType::Directory => "Directory",
        };

        self.conn.execute(
            "UPDATE nodes SET parent_id = ?2, name = ?3, node_type = ?4, size = ?5, hash = ?6, version = ?7, modified_at = ?8 WHERE id = ?1",
            params![
                node.id.to_string(),
                node.parent_id.map(|u| u.to_string()),
                node.name,
                node_type_str,
                node.size as i64,
                node.hash,
                node.version as i64,
                node.modified_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn delete_node(&self, id: &NodeId) -> RusqliteResult<()> {
        self.conn.execute("DELETE FROM nodes WHERE id = ?1", params![id.to_string()])?;
        Ok(())
    }

    pub fn get_unsynced_files(&self) -> RusqliteResult<Vec<Node>> {
        let mut stmt = self.conn.prepare(
            "SELECT n.id, n.parent_id, n.name, n.node_type, n.size, n.hash, n.version, n.created_at, n.modified_at 
             FROM nodes n 
             LEFT JOIN storage_objects s ON n.id = s.node_id 
             WHERE n.node_type = 'File' AND s.node_id IS NULL"
        )?;

        let node_iter = stmt.query_map([], |row| Self::map_node_row(row))?;

        let mut unsynced = Vec::new();
        for node in node_iter {
            unsynced.push(node?);
        }
        Ok(unsynced)
    }

    pub fn mark_file_as_synced(&self, node_id: &NodeId, remote_id: &str, size: u64, hash: &str) -> RusqliteResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO storage_objects (node_id, provider, remote_id, size, hash) 
             VALUES (?1, 'Telegram', ?2, ?3, ?4)",
            params![node_id.to_string(), remote_id, size as i64, hash],
        )?;

        self.conn.execute(
            "INSERT OR REPLACE INTO local_objects (node_id, device_id, local_path, state) 
             VALUES (?1, 'laptop-1', '', 'Cached')",
            params![node_id.to_string()],
        )?;

        Ok(())
    }

    pub fn get_remote_id(&self, node_id: &NodeId) -> RusqliteResult<Option<String>> {
        let mut stmt = self.conn.prepare("SELECT remote_id FROM storage_objects WHERE node_id = ?1")?;
        let mut rows = stmt.query(params![node_id.to_string()])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }
}
