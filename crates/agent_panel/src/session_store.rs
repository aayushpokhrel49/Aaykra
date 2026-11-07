use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use fs::{Fs, RemoveOptions};
use serde::{Deserialize, Serialize};

/// Metadata describing one saved conversation. Kept in a small JSON index so
/// session listings do not require reading every conversation file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredSessionMeta {
    pub id: String,
    pub title: String,
    pub agent_id: String,
    pub updated_at: u64,
}

pub fn sessions_dir() -> PathBuf {
    paths::data_dir().join("agent_panel_sessions")
}

pub fn new_session_id() -> String {
    format!(
        "{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    )
}

pub fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub async fn save_session(
    fs: Arc<dyn Fs>,
    dir: &Path,
    meta: &StoredSessionMeta,
    body: &str,
) -> anyhow::Result<()> {
    fs.create_dir(dir).await?;

    let mut metas = load_index(fs.clone(), dir).await.unwrap_or_default();
    if let Some(existing) = metas.iter_mut().find(|entry| entry.id == meta.id) {
        *existing = meta.clone();
    } else {
        metas.push(meta.clone());
    }
    metas.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    fs.write(&dir.join("index.json"), serde_json::to_vec(&metas)?.as_slice())
        .await?;

    fs.write(
        &dir.join(format!("session_{}.json", meta.id)),
        body.as_bytes(),
    )
    .await?;
    Ok(())
}

pub async fn load_index(
    fs: Arc<dyn Fs>,
    dir: &Path,
) -> anyhow::Result<Vec<StoredSessionMeta>> {
    let data = read_string(fs, &dir.join("index.json")).await?;
    Ok(serde_json::from_str(&data)?)
}

pub async fn load_body(
    fs: Arc<dyn Fs>,
    dir: &Path,
    id: &str,
) -> anyhow::Result<String> {
    read_string(fs, &dir.join(format!("session_{id}.json"))).await
}

pub async fn delete_session(
    fs: Arc<dyn Fs>,
    dir: &Path,
    id: &str,
) -> anyhow::Result<()> {
    let _ = fs
        .remove_file(&dir.join(format!("session_{id}.json")), RemoveOptions::default())
        .await;
    let metas = load_index(fs.clone(), dir).await.unwrap_or_default();
    let remaining: Vec<StoredSessionMeta> =
        metas.into_iter().filter(|entry| entry.id != id).collect();
    fs.write(&dir.join("index.json"), serde_json::to_vec(&remaining)?.as_slice())
        .await?;
    Ok(())
}

async fn read_string(fs: Arc<dyn Fs>, path: &Path) -> anyhow::Result<String> {
    let mut reader = fs.open_sync(path).await?;
    let mut contents = String::new();
    reader.read_to_string(&mut contents)?;
    Ok(contents)
}
