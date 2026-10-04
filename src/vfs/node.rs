use chrono::{DateTime, Utc};
use uuid::Uuid;

pub type NodeId  = Uuid;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum NodeType{
    File, 
    Directory,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Node{
    pub id: NodeId,
    pub parent_id: Option<NodeId>,
    pub name: String,
    pub node_type: NodeType,
    pub size: u64,
    pub hash: String,
    pub version: u64,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProviderType {
    Telegram,
    // Add other providers here if needed
}

#[derive(Debug, Clone)]
pub struct StorageObject {
    pub node_id: NodeId,
    pub provider: ProviderType,
    pub remote_id: String,
    pub size: u64,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LocalState {
    Cached,
    Syncing,
    NotCached,
}

#[derive(Debug, Clone)]
pub struct LocalObject {
    pub node_id: NodeId,
    pub device_id: String,
    pub local_path: String,
    pub state: LocalState,
}