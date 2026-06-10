use crate::models::FileMeta;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use thiserror::Error;
use walkdir::WalkDir;

#[derive(Error, Debug)]
pub enum FsError {
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub fn default_ignore_names() -> &'static [&'static str] {
    &[
        ".DS_Store",
        ".git",
        "node_modules",
        ".stignore",
        ".sync-state.json",
    ]
}

pub fn should_ignore(name: &str, extra_ignore: &[String]) -> bool {
    if default_ignore_names().contains(&name) {
        return true;
    }
    if name.starts_with('.') {
        return false;
    }
    extra_ignore.iter().any(|p| name.contains(p))
}

pub fn resolve_safe_dir(input_path: &str) -> Result<PathBuf, FsError> {
    let trimmed = input_path.trim();
    if trimmed.is_empty() {
        return Err(FsError::Message("路径不能为空".into()));
    }
    let resolved = PathBuf::from(trimmed).canonicalize().map_err(|_| {
        FsError::Message(format!("目录不存在: {trimmed}"))
    })?;
    let meta = fs::metadata(&resolved)?;
    if !meta.is_dir() {
        return Err(FsError::Message(format!("不是文件夹: {}", resolved.display())));
    }
    Ok(resolved)
}

pub fn resolve_existing_path(input_path: &str) -> Result<PathBuf, FsError> {
    let trimmed = input_path.trim();
    if trimmed.is_empty() {
        return Err(FsError::Message("路径不能为空".into()));
    }
    let path = PathBuf::from(trimmed);
    if !path.exists() {
        return Err(FsError::Message(format!("路径不存在: {trimmed}")));
    }
    Ok(path.canonicalize()?)
}

pub fn walk_files(root: &Path, extra_ignore: &[String]) -> HashMap<String, FileMeta> {
    let mut files = HashMap::new();
    for entry in WalkDir::new(root).follow_links(false).into_iter().filter_map(|e| e.ok()) {
        let ent = entry.path();
        if ent == root {
            continue;
        }
        let rel = match ent.strip_prefix(root) {
            Ok(r) => r,
            Err(_) => continue,
        };
        if rel.components().any(|c| {
            if let std::path::Component::Normal(name) = c {
                should_ignore(&name.to_string_lossy(), extra_ignore)
            } else {
                false
            }
        }) {
            continue;
        }
        if !ent.is_file() {
            continue;
        }
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if let Ok(st) = fs::metadata(ent) {
            let mtime = st
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as f64)
                .unwrap_or(0.0);
            files.insert(
                rel_str.clone(),
                FileMeta {
                    relative_path: rel_str,
                    absolute_path: ent.to_string_lossy().into_owned(),
                    size: st.len(),
                    mtime,
                    md5: None,
                    error: None,
                },
            );
        }
    }
    files
}

pub fn md5_file(file_path: &Path) -> Result<String, std::io::Error> {
    let mut file = fs::File::open(file_path)?;
    let mut context = md5::Context::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        context.consume(&buffer[..n]);
    }
    Ok(format!("{:x}", context.compute()))
}

pub async fn hash_files(
    file_map: HashMap<String, FileMeta>,
) -> HashMap<String, FileMeta> {
    let mut result = HashMap::new();
    for (rel, meta) in file_map {
        let path = PathBuf::from(&meta.absolute_path);
        let updated = match md5_file(&path) {
            Ok(hash) => FileMeta {
                md5: Some(hash),
                ..meta
            },
            Err(_) => FileMeta {
                md5: None,
                error: Some("无法读取".into()),
                ..meta
            },
        };
        result.insert(rel, updated);
    }
    result
}

pub fn ensure_dir_for_file(file_path: &Path) -> Result<(), std::io::Error> {
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

pub fn copy_file_safe(src: &Path, dst: &Path) -> Result<(), std::io::Error> {
    ensure_dir_for_file(dst)?;
    fs::copy(src, dst)?;
    Ok(())
}

pub fn unique_dir_path(parent_dir: &Path, base_name: &str) -> PathBuf {
    let candidate = parent_dir.join(base_name);
    if !candidate.exists() {
        return candidate;
    }
    let mut i = 1;
    loop {
        let next = parent_dir.join(format!("{base_name}_{i}"));
        if !next.exists() {
            return next;
        }
        i += 1;
    }
}

pub fn unique_file_path(dir: &Path, base_name: &str, ext: &str) -> PathBuf {
    let candidate = dir.join(format!("{base_name}{ext}"));
    if !candidate.exists() {
        return candidate;
    }
    let mut i = 1;
    loop {
        let next = dir.join(format!("{base_name}_{i}{ext}"));
        if !next.exists() {
            return next;
        }
        i += 1;
    }
}

pub fn safe_file_name(name: &str) -> String {
    crate::rename_ops::sanitize_name(name)
        .chars()
        .take(255)
        .collect::<String>()
        .trim()
        .to_string()
        .if_empty_then("untitled")
}

trait IfEmpty {
    fn if_empty_then(self, fallback: &str) -> String;
}

impl IfEmpty for String {
    fn if_empty_then(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_string()
        } else {
            self
        }
    }
}
