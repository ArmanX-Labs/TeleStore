import React, { useState, useEffect, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Cloud, Folder, File, FileText, FileImage, Trash2, Upload, Plus, ChevronLeft, LogOut } from 'lucide-react';
import { listDirectory, createFolder, deleteItem, openFile, uploadPhysicalFile } from './api';
import Login from './Login';
import './App.css';

export default function App() {
  const [isAuthorized, setIsAuthorized] = useState(null);
  const [currentPath, setCurrentPath] = useState("/");
  const [files, setFiles] = useState([]);
  const [uploading, setUploading] = useState(false);
  const fileInputRef = useRef(null);

  useEffect(() => {
    const token = localStorage.getItem('mycloud_token') || "";
    fetch('/api/auth/status', {
      headers: { 'Authorization': `Bearer ${token}` }
    })
      .then(res => res.json())
      .then(data => setIsAuthorized(data.authorized))
      .catch(err => {
        console.error(err);
        setIsAuthorized(false);
      });
  }, []);

  useEffect(() => {
    if (isAuthorized) {
      loadFiles();
    }
  }, [currentPath, isAuthorized]);

  useEffect(() => {
    if (!isAuthorized) return;
    let unlistenPromise = null;
    
    if (window.__TAURI__ && window.__TAURI__.event && window.__TAURI__.path) {
      unlistenPromise = window.__TAURI__.event.listen('tauri://file-drop', async (event) => {
        const filePaths = event.payload;
        for (const filePath of filePaths) {
          const name = await window.__TAURI__.path.basename(filePath);
          doUpload(filePath, null, name);
        }
      });
    }

    return () => {
      if (unlistenPromise) {
        unlistenPromise.then(unlisten => {
          if (unlisten) unlisten();
        }).catch(e => console.error("Failed to unlisten:", e));
      }
    };
  }, [currentPath]);

  const loadFiles = async () => {
    try {
      const data = await listDirectory(currentPath);
      setFiles(data);
    } catch (e) {
      console.error(e);
      // Dummy data if backend is dead
      if (e.toString().includes('backend')) {
         setFiles([{name: 'Projects', node_type: 'Directory', size: 0, id: 1}]);
      }
    }
  };

  const formatBytes = (bytes) => {
    if (!+bytes) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
  };

  const getIcon = (node) => {
    if (node.node_type === "Directory") return <Folder size={48} color="var(--text-secondary)" strokeWidth={1} />;
    const ext = node.name.split('.').pop().toLowerCase();
    if (['jpg', 'png', 'gif', 'jpeg'].includes(ext)) return <FileImage size={48} color="var(--text-secondary)" strokeWidth={1} />;
    if (['txt', 'md', 'pdf'].includes(ext)) return <FileText size={48} color="var(--text-secondary)" strokeWidth={1} />;
    return <File size={48} color="var(--text-secondary)" strokeWidth={1} />;
  };

  const handleUploadClick = async () => {
    if (window.__TAURI__ && window.__TAURI__.dialog) {
      try {
        const selected = await window.__TAURI__.dialog.open({ multiple: false });
        if (selected) {
          const name = await window.__TAURI__.path.basename(selected);
          doUpload(selected, null, name);
        }
      } catch (e) {
        console.error(e);
      }
    } else {
      fileInputRef.current?.click();
    }
  };

  const handleWebFileChange = (e) => {
    const file = e.target.files[0];
    if (file) {
      doUpload(null, file, file.name);
    }
  };

  const doUpload = async (sourcePath, fileObj, fileName) => {
    setUploading(true);
    setFiles(prev => [...prev, { name: fileName, size: 0, node_type: "File", isUploading: true, id: 'temp' }]);
    try {
      await uploadPhysicalFile(sourcePath, fileObj, fileName, currentPath);
      loadFiles();
    } catch (e) {
      alert("Upload failed: " + e);
      loadFiles();
    } finally {
      setUploading(false);
    }
  };

  const handleCreateFolder = async () => {
    const name = window.prompt("Folder name:");
    if (name && name.trim()) {
      const targetPath = currentPath === "/" ? `/${name}` : `${currentPath}/${name}`;
      try {
        await createFolder(targetPath);
        loadFiles();
      } catch (e) {
        alert(e);
      }
    }
  };

  const handleDelete = async (e, file) => {
    e.stopPropagation();
    
    let confirmed = false;
    if (window.__TAURI__) {
       confirmed = await window.__TAURI__.dialog.ask(`Delete ${file.name}?`);
    } else {
       confirmed = window.confirm(`Delete ${file.name}?`);
    }
    
    if (confirmed) {
      const targetPath = currentPath === "/" ? `/${file.name}` : `${currentPath}/${file.name}`;
      await deleteItem(targetPath);
      loadFiles();
    }
  };

  const handleFileClick = async (file) => {
    if (file.isUploading) return;
    if (file.node_type === "Directory") {
      const newPath = currentPath === "/" ? `/${file.name}` : `${currentPath}/${file.name}`;
      setCurrentPath(newPath);
    } else {
      await openFile(file.id, file.name);
    }
  };

  const handleBack = () => {
    const segments = currentPath.split('/').filter(p => p.length > 0);
    segments.pop();
    setCurrentPath(segments.length > 0 ? "/" + segments.join("/") : "/");
  };

  if (isAuthorized === null) {
    return <div style={{ display: 'flex', height: '100vh', alignItems: 'center', justifyContent: 'center' }}>Loading...</div>;
  }

  if (isAuthorized === false) {
    return <Login onLoginSuccess={() => setIsAuthorized(true)} />;
  }

  return (
    <div className="app-container">
      <div className="sidebar">
        <div className="logo">
          <Cloud color="var(--accent)" /> MyCloud
        </div>
        <div className="nav-item active"><Folder size={20} /> All Files</div>
        <div className="nav-item"><Trash2 size={20} /> Trash</div>
        <div className="nav-item" style={{ marginTop: 'auto' }} onClick={() => { localStorage.removeItem('mycloud_token'); setIsAuthorized(false); }}><LogOut size={20} /> Log Out</div>
      </div>

      <div className="main-content">
        <div className="header">
          <div className="path-display">
            {currentPath !== "/" && (
              <button className="icon-btn" onClick={handleBack}>
                <ChevronLeft size={24} />
              </button>
            )}
            {currentPath === "/" ? "My Files" : currentPath}
          </div>
          <div className="actions">
            <input 
              type="file" 
              ref={fileInputRef} 
              style={{ display: 'none' }} 
              onChange={handleWebFileChange} 
            />
            <button className="btn btn-secondary" onClick={handleCreateFolder}>
              <Plus size={18} /> New Folder
            </button>
            <button className="btn btn-primary" onClick={handleUploadClick}>
              <Upload size={18} /> Upload
            </button>
          </div>
        </div>

        {files.length === 0 ? (
          <motion.div 
            initial={{ opacity: 0 }} animate={{ opacity: 1 }} 
            className="empty-state"
          >
            <Folder size={64} color="rgba(255,255,255,0.05)" />
            <p>This folder is empty</p>
          </motion.div>
        ) : (
          <div className="file-grid">
            <AnimatePresence>
              {files.map((file, i) => (
                <motion.div
                  key={file.id || file.name}
                  layout
                  initial={{ opacity: 0, scale: 0.9, y: 10 }}
                  animate={{ opacity: 1, scale: 1, y: 0 }}
                  exit={{ opacity: 0, scale: 0.9 }}
                  transition={{ duration: 0.2 }}
                  className="file-card"
                  onClick={() => handleFileClick(file)}
                >
                  <div className="file-icon">{getIcon(file)}</div>
                  <div className="file-info">
                    <div className="file-name" title={file.name}>{file.name}</div>
                    <div className="file-meta">
                      {file.isUploading ? "Uploading..." : (file.node_type === "Directory" ? "Folder" : formatBytes(file.size))}
                    </div>
                  </div>
                  {!file.isUploading && (
                    <button className="delete-btn" onClick={(e) => handleDelete(e, file)}>
                      <Trash2 size={16} />
                    </button>
                  )}
                </motion.div>
              ))}
            </AnimatePresence>
          </div>
        )}
      </div>
    </div>
  );
}
