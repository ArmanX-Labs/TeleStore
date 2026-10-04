const isTauri = window.__TAURI__ !== undefined;
const getToken = () => localStorage.getItem('mycloud_token') || "";

export async function listDirectory(path) {
  const token = getToken();
  if (isTauri) {
    const { invoke } = window.__TAURI__.tauri;
    return await invoke('list_directory', { token, path });
  } else {
    const res = await fetch(`/api/list?path=${encodeURIComponent(path)}`, {
      headers: { 'Authorization': `Bearer ${token}` }
    });
    if (!res.ok) throw new Error(await res.text());
    return await res.json();
  }
}

export async function createFolder(path) {
  const token = getToken();
  if (isTauri) {
    const { invoke } = window.__TAURI__.tauri;
    return await invoke('create_folder', { token, path });
  } else {
    const res = await fetch('/api/create_folder', {
      method: 'POST',
      headers: { 
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${token}`
      },
      body: JSON.stringify({ path })
    });
    if (!res.ok) throw new Error(await res.text());
  }
}

export async function deleteItem(path) {
  const token = getToken();
  if (isTauri) {
    const { invoke } = window.__TAURI__.tauri;
    return await invoke('delete_item', { token, path });
  } else {
    const res = await fetch('/api/delete', {
      method: 'POST',
      headers: { 
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${token}`
      },
      body: JSON.stringify({ path })
    });
    if (!res.ok) throw new Error(await res.text());
  }
}

export async function openFile(id, name) {
  const token = getToken();
  if (isTauri) {
    const { invoke } = window.__TAURI__.tauri;
    return await invoke('open_file', { token, id, name });
  } else {
    // For browser downloads we can't easily pass Authorization header in window.open,
    // so we pass it in query params, or fetch as blob. 
    // Wait, let's fetch as blob to pass Authorization header!
    const res = await fetch(`/api/download?id=${encodeURIComponent(id)}&name=${encodeURIComponent(name)}`, {
      headers: { 'Authorization': `Bearer ${token}` }
    });
    if (!res.ok) throw new Error(await res.text());
    const blob = await res.blob();
    const url = window.URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = name;
    a.click();
    window.URL.revokeObjectURL(url);
  }
}

export async function uploadPhysicalFile(sourcePath, fileObj, fileName, parentPath) {
  const token = getToken();
  if (isTauri) {
    const { invoke } = window.__TAURI__.tauri;
    return await invoke('upload_physical_file', {
      token,
      sourcePath,
      fileName,
      parentPath
    });
  } else {
    const formData = new FormData();
    formData.append('parentPath', parentPath);
    formData.append('file', fileObj, fileName);
    const res = await fetch('/api/upload', {
      method: 'POST',
      headers: { 'Authorization': `Bearer ${token}` },
      body: formData
    });
    if (!res.ok) throw new Error(await res.text());
  }
}
