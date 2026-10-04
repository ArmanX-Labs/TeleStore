use grammers_client::{Client, SignInError, message::InputMessage, tl};
use grammers_client::client::{LoginToken, PasswordToken};
use grammers_mtsender::SenderPool;
use grammers_session::storages::SqliteSession;
use std::sync::Arc;

pub struct TelegramProvider {
    api_id: i32,
    api_hash: String,
    client: Option<Client>,
}

impl TelegramProvider {
    pub fn new(api_id: i32, api_hash: String) -> Self {
        Self {
            api_id,
            api_hash,
            client: None,
        }
    }

    pub async fn init(&mut self, session_file: &str) -> Result<(), String> {
        let session = Arc::new(SqliteSession::open(session_file).await.map_err(|e| e.to_string())?);
        
        let SenderPool {
            runner,
            updates: _,
            handle,
        } = SenderPool::new(Arc::clone(&session), self.api_id);
        
        let client = Client::new(handle.clone());
        tokio::spawn(runner.run());

        self.client = Some(client);
        Ok(())
    }

    pub async fn is_authorized(&self) -> Result<bool, String> {
        self.client.as_ref().unwrap().is_authorized().await.map_err(|e| e.to_string())
    }

    pub async fn request_login_code(&self, phone: &str) -> Result<LoginToken, String> {
        self.client.as_ref().unwrap().request_login_code(phone, &self.api_hash).await.map_err(|e| e.to_string())
    }

    pub async fn sign_in(&self, token: &LoginToken, code: &str) -> Result<Option<PasswordToken>, String> {
        match self.client.as_ref().unwrap().sign_in(token, code).await {
            Err(SignInError::PasswordRequired(pt)) => Ok(Some(pt)),
            Err(e) => Err(e.to_string()),
            Ok(_) => Ok(None),
        }
    }

    pub async fn check_password(&self, token: PasswordToken, password: &str) -> Result<(), String> {
        self.client.as_ref().unwrap().check_password(token, password).await.map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn upload_bytes(&self, filename: &str, data: Vec<u8>) -> Result<String, String> {
        let client = self.client.as_ref().unwrap();
        let size = data.len();
        let mut cursor = std::io::Cursor::new(data);
        
        println!("  [MTProto] Uploading '{}' ({} bytes)...", filename, size);
        let uploaded = client
            .upload_stream(&mut cursor, size, filename.to_string())
            .await
            .map_err(|e| format!("Upload stream error: {}", e))?;
            
        let me = &tl::types::InputPeerSelf {};
        
        println!("  [MTProto] Sending to Saved Messages...");
        let msg = client.send_message(
            me,
            InputMessage::new().text(filename).document(uploaded)
        ).await.map_err(|e| format!("Send message error: {}", e))?;
        
        Ok(msg.id().to_string())
    }

    pub async fn download_to_file(&self, remote_id: &str, dest_path: &str) -> Result<(), String> {
        let client = self.client.as_ref().unwrap();
        let msg_id: i32 = remote_id.parse().map_err(|_| "Invalid remote ID format")?;
        
        let me = &tl::types::InputPeerSelf {};
        
        let mut messages = client.get_messages_by_id(me, &[msg_id]).await.map_err(|e| e.to_string())?;
        
        let msg = messages.pop().flatten().ok_or_else(|| "Message not found in Saved Messages".to_string())?;
        let media = msg.media().ok_or_else(|| "Message has no media attached".to_string())?;
        
        println!("  [MTProto] Downloading file from message ID {} to {}...", msg_id, dest_path);
        
        client.download_media(&media, dest_path).await.map_err(|e| format!("Download error: {}", e))?;
        
        Ok(())
    }
}
