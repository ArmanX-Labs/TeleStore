use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;
use std::time::{SystemTime, UNIX_EPOCH};
use std::fs;
use std::collections::HashMap;
use crate::vfs::filesystem::FileSystem;
use crate::sync::engine::SyncEngine;
use crate::provider::telegram::TelegramProvider;
use grammers_client::client::{LoginToken, PasswordToken};
use std::sync::Mutex;
use tokio::sync::RwLock;

pub struct UserSession {
    pub fs: Mutex<FileSystem>,
    pub sync_engine: SyncEngine,
}

pub struct AuthState {
    pub login_token: Option<LoginToken>,
    pub password_token: Option<PasswordToken>,
    pub provider: TelegramProvider,
    pub phone: String,
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct PersistedTokens {
    pub phone_to_token: HashMap<String, String>,
    pub token_to_phone: HashMap<String, String>,
    pub token_timestamps: HashMap<String, u64>,
}

pub struct SessionManager {
    // Session token -> UserSession
    pub sessions: RwLock<HashMap<String, Arc<UserSession>>>,
    // Phone number -> AuthState
    pub auth_sessions: RwLock<HashMap<String, AuthState>>,
    
    pub persisted: RwLock<PersistedTokens>,
    
    pub api_id: i32,
    pub api_hash: String,
}

impl SessionManager {
    pub fn new(api_id: i32, api_hash: String) -> Self {
        let persisted = if let Ok(data) = fs::read_to_string("data/sessions.json") {
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            PersistedTokens::default()
        };
        
        Self {
            sessions: RwLock::new(HashMap::new()),
            auth_sessions: RwLock::new(HashMap::new()),
            persisted: RwLock::new(persisted),
            api_id,
            api_hash,
        }
    }
    
    async fn save_persisted(&self) {
        let lock = self.persisted.read().await;
        if let Ok(data) = serde_json::to_string_pretty(&*lock) {
            let _ = fs::write("data/sessions.json", data);
        }
    }

    pub async fn get_session(&self, token: &str) -> Option<Arc<UserSession>> {
        {
            let lock = self.sessions.read().await;
            if let Some(session) = lock.get(token) {
                return Some(session.clone());
            }
        }
        
        // Check persisted
        let phone = {
            let mut lock = self.persisted.write().await;
            if let Some(timestamp) = lock.token_timestamps.get(token) {
                let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
                if now - timestamp > 2 * 24 * 3600 { // 2 days
                    // Expired
                    let phone = lock.token_to_phone.remove(token).unwrap_or_default();
                    lock.phone_to_token.remove(&phone);
                    lock.token_timestamps.remove(token);
                    return None;
                }
                lock.token_to_phone.get(token).cloned()
            } else {
                None
            }
        };
        
        if let Some(phone) = phone {
            // Reconstruct session
            let mut provider = TelegramProvider::new(self.api_id, self.api_hash.clone());
            let session_file = format!("data/session_{}.session", phone);
            if provider.init(&session_file).await.is_ok() && provider.is_authorized().await.unwrap_or(false) {
                let db_file = format!("data/db_{}.sqlite", phone);
                let fs = FileSystem::new_with_db(&db_file).unwrap();
                let sync_engine = SyncEngine::new(provider);
                
                let user_session = Arc::new(UserSession {
                    fs: std::sync::Mutex::new(fs),
                    sync_engine,
                });
                
                self.sessions.write().await.insert(token.to_string(), user_session.clone());
                return Some(user_session);
            }
        }
        
        None
    }

    pub async fn add_session(&self, phone: String, token: String, session: Arc<UserSession>) {
        {
            let mut lock = self.persisted.write().await;
            if let Some(old_token) = lock.phone_to_token.insert(phone.clone(), token.clone()) {
                lock.token_to_phone.remove(&old_token);
                lock.token_timestamps.remove(&old_token);
                self.sessions.write().await.remove(&old_token);
            }
            lock.token_to_phone.insert(token.clone(), phone);
            lock.token_timestamps.insert(token.clone(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs());
        }
        
        self.save_persisted().await;
        
        let mut session_lock = self.sessions.write().await;
        session_lock.insert(token, session);
    }
    
    pub async fn get_auth(&self, phone: &str) -> Option<AuthState> {
        let mut lock = self.auth_sessions.write().await;
        lock.remove(phone)
    }
    
    pub async fn save_auth(&self, phone: String, state: AuthState) {
        let mut lock = self.auth_sessions.write().await;
        lock.insert(phone, state);
    }
}
