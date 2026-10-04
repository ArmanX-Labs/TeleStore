use axum::{
    routing::{get, post},
    Router,
    extract::{State, Query, Multipart},
    Json,
};
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use crate::AppState;
use crate::vfs::node::Node;

#[derive(Serialize)]
pub struct AuthStatus {
    authorized: bool,
}

#[derive(Deserialize)]
pub struct AuthPhoneRequest {
    phone: String,
}

#[derive(Deserialize)]
pub struct AuthCodeRequest {
    pub phone: String,
    pub code: String,
}

#[derive(Deserialize)]
pub struct AuthPasswordRequest {
    pub phone: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct PathQuery {
    path: String,
}

#[derive(Deserialize)]
pub struct DeleteQuery {
    path: String,
}

#[derive(Deserialize)]
pub struct DownloadQuery {
    id: String,
    name: String,
}

pub async fn start_server(app_state: Arc<AppState>) {
    let app = Router::new()
        .route("/api/list", get(api_list))
        .route("/api/create_folder", post(api_create_folder))
        .route("/api/delete", post(api_delete))
        .route("/api/upload", post(api_upload))
        .route("/api/download", get(api_download))
        .route("/api/auth/status", get(api_auth_status))
        .route("/api/auth/send_code", post(api_auth_send_code))
        .route("/api/auth/sign_in", post(api_auth_sign_in))
        .route("/api/auth/check_password", post(api_auth_check_password))
        // Serve the frontend build
        .fallback_service(
            ServeDir::new("frontend/dist")
                .fallback(ServeFile::new("frontend/dist/index.html"))
        )
        .with_state(app_state)
        .layer(CorsLayer::permissive());

    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{}", port);
    println!("Starting Web App server on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn api_list(State(state): State<Arc<AppState>>, headers: axum::http::HeaderMap, Query(query): Query<PathQuery>) -> Result<Json<Vec<Node>>, (axum::http::StatusCode, String)> {
    let token = extract_token(&headers)?;
    let session = state.sessions.get_session(&token).await.ok_or((axum::http::StatusCode::UNAUTHORIZED, "Invalid session".to_string()))?;
    
    let fs = session.fs.lock().unwrap();
    fs.list(&query.path).map(Json).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))
}

async fn api_create_folder(State(state): State<Arc<AppState>>, headers: axum::http::HeaderMap, Json(query): Json<PathQuery>) -> Result<(), (axum::http::StatusCode, String)> {
    let token = extract_token(&headers)?;
    let session = state.sessions.get_session(&token).await.ok_or((axum::http::StatusCode::UNAUTHORIZED, "Invalid session".to_string()))?;
    
    let mut fs = session.fs.lock().unwrap();
    fs.mkdir(&query.path).map(|_| ()).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))
}

async fn api_delete(State(state): State<Arc<AppState>>, headers: axum::http::HeaderMap, Json(query): Json<DeleteQuery>) -> Result<(), (axum::http::StatusCode, String)> {
    let token = extract_token(&headers)?;
    let session = state.sessions.get_session(&token).await.ok_or((axum::http::StatusCode::UNAUTHORIZED, "Invalid session".to_string()))?;
    
    let mut fs = session.fs.lock().unwrap();
    fs.delete(&query.path).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))
}

async fn api_upload(State(state): State<Arc<AppState>>, headers: axum::http::HeaderMap, mut multipart: Multipart) -> Result<(), (axum::http::StatusCode, String)> {
    let token = extract_token(&headers)?;
    let session = state.sessions.get_session(&token).await.ok_or((axum::http::StatusCode::UNAUTHORIZED, "Invalid session".to_string()))?;
    
    let mut parent_path = String::new();
    let mut file_name = String::new();
    let mut file_data = Vec::new();

    while let Some(field) = multipart.next_field().await.map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e.to_string()))? {
        let name = field.name().unwrap_or("").to_string();
        if name == "parentPath" {
            parent_path = field.text().await.map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e.to_string()))?;
        } else if name == "file" {
            file_name = field.file_name().unwrap_or("unknown").to_string();
            file_data = field.bytes().await.map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e.to_string()))?.to_vec();
        }
    }

    if file_data.is_empty() {
        return Err((axum::http::StatusCode::BAD_REQUEST, "No file data".into()));
    }

    let node = {
        let mut fs = session.fs.lock().unwrap();
        let vfs_path = if parent_path == "/" {
            format!("/{}", file_name)
        } else {
            format!("{}/{}", parent_path, file_name)
        };
        
        let _ = fs.create_file(&vfs_path);
        fs.write(&vfs_path, &file_data).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))?;

        let id = fs.resolve_path(&vfs_path).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))?;
        fs.db().get_node(&id).unwrap().unwrap()
    };

    // Upload to Telegram
    let remote_id = session.sync_engine.process_upload(&node).await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Mark synced
    {
        let fs = session.fs.lock().unwrap();
        fs.db().mark_file_as_synced(&node.id, &remote_id, node.size, &node.hash)
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    Ok(())
}

async fn api_download(State(state): State<Arc<AppState>>, headers: axum::http::HeaderMap, Query(query): Query<DownloadQuery>) -> Result<impl axum::response::IntoResponse, (axum::http::StatusCode, String)> {
    let token = extract_token(&headers)?;
    let session = state.sessions.get_session(&token).await.ok_or((axum::http::StatusCode::UNAUTHORIZED, "Invalid session".to_string()))?;
    
    let cache_path = format!(".mycloud_cache/{}", query.id);
    if !std::path::Path::new(&cache_path).exists() {
        let node_uuid = uuid::Uuid::parse_str(&query.id).map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e.to_string()))?;
        
        let remote_id = {
            let fs = session.fs.lock().unwrap();
            fs.db().get_remote_id(&node_uuid)
                .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
                .ok_or_else(|| (axum::http::StatusCode::NOT_FOUND, "File not synced".to_string()))?
        };
        
        let _ = std::fs::create_dir_all(".mycloud_cache");
        session.sync_engine.download_to_file(&remote_id, &cache_path).await.map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))?;
    }
    
    let data = tokio::fs::read(&cache_path).await.map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    use axum::http::header;
    let headers = [
        (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", query.name)),
        (header::CONTENT_TYPE, "application/octet-stream".to_string()),
    ];
    
    Ok((headers, data))
}

use crate::provider::telegram::TelegramProvider;
use crate::vfs::filesystem::FileSystem;
use crate::sync::engine::SyncEngine;

fn extract_token(headers: &axum::http::HeaderMap) -> Result<String, (axum::http::StatusCode, String)> {
    headers.get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer ").map(|s| s.to_string()))
        .ok_or((axum::http::StatusCode::UNAUTHORIZED, "Missing token".to_string()))
}

async fn api_auth_status(State(state): State<Arc<AppState>>, headers: axum::http::HeaderMap) -> Result<Json<AuthStatus>, (axum::http::StatusCode, String)> {
    if let Ok(token) = extract_token(&headers) {
        let is_auth = state.sessions.get_session(&token).await.is_some();
        Ok(Json(AuthStatus { authorized: is_auth }))
    } else {
        Ok(Json(AuthStatus { authorized: false }))
    }
}

async fn api_auth_send_code(State(state): State<Arc<AppState>>, Json(req): Json<AuthPhoneRequest>) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let mut provider = TelegramProvider::new(state.api_id, state.api_hash.clone());
    let session_file = format!("data/session_{}.session", req.phone);
    provider.init(&session_file).await.map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))?;

    if provider.is_authorized().await.unwrap_or(false) {
        let db_file = format!("data/db_{}.sqlite", req.phone);
        let fs = FileSystem::new_with_db(&db_file).unwrap();
        let sync_engine = SyncEngine::new(provider);
        
        let user_session = Arc::new(crate::session::UserSession {
            fs: std::sync::Mutex::new(fs),
            sync_engine,
        });
        
        let session_token = uuid::Uuid::new_v4().to_string();
        state.sessions.add_session(req.phone.clone(), session_token.clone(), user_session).await;
        
        return Ok(Json(serde_json::json!({ "token": session_token })));
    }

    let token = provider.request_login_code(&req.phone).await.map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e))?;
    
    let auth_state = crate::session::AuthState {
        login_token: Some(token),
        password_token: None,
        provider,
        phone: req.phone.clone(),
    };
    
    state.sessions.save_auth(req.phone, auth_state).await;
    Ok(Json(serde_json::json!({ "requires_code": true })))
}

async fn api_auth_sign_in(State(state): State<Arc<AppState>>, Json(req): Json<AuthCodeRequest>) -> Result<Json<Option<String>>, (axum::http::StatusCode, String)> {
    let mut auth_state = state.sessions.get_auth(&req.phone).await.ok_or_else(|| (axum::http::StatusCode::BAD_REQUEST, "No login session".to_string()))?;
    
    let token = auth_state.login_token.take().ok_or_else(|| (axum::http::StatusCode::BAD_REQUEST, "No login session started".to_string()))?;
    
    match auth_state.provider.sign_in(&token, &req.code).await {
        Ok(Some(password_token)) => {
            auth_state.password_token = Some(password_token);
            state.sessions.save_auth(req.phone.clone(), auth_state).await;
            Ok(Json(None)) // Needs password
        }
        Ok(None) => {
            // Successfully logged in!
            
            // Create UserSession
            let db_file = format!("data/db_{}.sqlite", req.phone);
            let fs = FileSystem::new_with_db(&db_file).unwrap();
            let sync_engine = SyncEngine::new(auth_state.provider);
            
            let user_session = Arc::new(crate::session::UserSession {
                fs: std::sync::Mutex::new(fs),
                sync_engine,
            });
            
            let session_token = uuid::Uuid::new_v4().to_string();
            state.sessions.add_session(req.phone.clone(), session_token.clone(), user_session).await;
            
            Ok(Json(Some(session_token)))
        }
        Err(e) => Err((axum::http::StatusCode::BAD_REQUEST, e)),
    }
}

async fn api_auth_check_password(State(state): State<Arc<AppState>>, Json(req): Json<AuthPasswordRequest>) -> Result<Json<String>, (axum::http::StatusCode, String)> {
    let mut auth_state = state.sessions.get_auth(&req.phone).await.ok_or_else(|| (axum::http::StatusCode::BAD_REQUEST, "No login session".to_string()))?;
    let token = auth_state.password_token.take().ok_or_else(|| (axum::http::StatusCode::BAD_REQUEST, "No password session started".to_string()))?;
    
    auth_state.provider.check_password(token, &req.password).await.map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e))?;
            
    // Create UserSession
    let db_file = format!("data/db_{}.sqlite", req.phone);
    let fs = FileSystem::new_with_db(&db_file).unwrap();
    let sync_engine = SyncEngine::new(auth_state.provider);
    
    let user_session = Arc::new(crate::session::UserSession {
        fs: std::sync::Mutex::new(fs),
        sync_engine,
    });
    
    let session_token = uuid::Uuid::new_v4().to_string();
    state.sessions.add_session(req.phone.clone(), session_token.clone(), user_session).await;
    
    Ok(Json(session_token))
}
