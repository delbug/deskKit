use crate::models::CompareExtensionMode;
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
    walk_files_filtered(root, extra_ignore, WalkFilterOptions::default())
}

#[derive(Debug, Clone, Default)]
pub struct WalkFilterOptions {
    pub min_size_bytes: u64,
    pub extension_mode: CompareExtensionMode,
    pub extensions: Vec<String>,
}

impl WalkFilterOptions {
    pub fn from_min_size_kb(min_size_kb: Option<u64>) -> Self {
        Self {
            min_size_bytes: min_size_kb.unwrap_or(0).saturating_mul(1024),
            ..Default::default()
        }
    }
}

pub fn normalize_extension(ext: &str) -> String {
    let s = ext.trim().trim_start_matches('.').to_ascii_lowercase();
    if s == "无后缀" || s == "(无后缀)" {
        String::new()
    } else {
        s
    }
}

pub fn extension_key(relative_path: &str) -> String {
    let name = Path::new(relative_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(relative_path);
    match name.rfind('.') {
        Some(i) if i > 0 && i < name.len() - 1 => name[i + 1..].to_ascii_lowercase(),
        _ => String::new(),
    }
}

pub fn matches_extension_filter(
    relative_path: &str,
    mode: CompareExtensionMode,
    extensions: &[String],
) -> bool {
    if mode == CompareExtensionMode::None || extensions.is_empty() {
        return true;
    }
    let allowed: Vec<String> = extensions.iter().map(|e| normalize_extension(e)).collect();
    let file_key = extension_key(relative_path);
    match mode {
        CompareExtensionMode::Include => allowed.contains(&file_key),
        CompareExtensionMode::Exclude => !allowed.contains(&file_key),
        CompareExtensionMode::None => true,
    }
}

pub fn walk_files_filtered(
    root: &Path,
    extra_ignore: &[String],
    options: WalkFilterOptions,
) -> HashMap<String, FileMeta> {
    let WalkFilterOptions {
        min_size_bytes,
        extension_mode,
        extensions,
    } = options;
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
        if !matches_extension_filter(&rel_str, extension_mode, &extensions) {
            continue;
        }
        if let Ok(st) = fs::metadata(ent) {
            if min_size_bytes > 0 && st.len() < min_size_bytes {
                continue;
            }
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

pub fn file_meta_for_path(path: &Path) -> Result<FileMeta, FsError> {
    let resolved = path.canonicalize().map_err(|e| FsError::Io(e))?;
    if !resolved.is_file() {
        return Err(FsError::Message(format!("不是文件: {}", resolved.display())));
    }
    let st = fs::metadata(&resolved)?;
    let mtime = st
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0);
    let name = resolved
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string();
    Ok(FileMeta {
        relative_path: name,
        absolute_path: resolved.to_string_lossy().into_owned(),
        size: st.len(),
        mtime,
        md5: None,
        error: None,
    })
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
    let mut i = 2;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp_file(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(bytes).unwrap();
        path
    }

    #[test]
    fn walk_files_filtered_skips_files_below_min_size() {
        let root = tempfile::tempdir().unwrap();
        write_temp_file(root.path(), "tiny.txt", b"x");
        write_temp_file(root.path(), "large.bin", &vec![0u8; 2048]);

        let all = walk_files_filtered(root.path(), &[], WalkFilterOptions::default());
        assert_eq!(all.len(), 2);

        let filtered = walk_files_filtered(
            root.path(),
            &[],
            WalkFilterOptions {
                min_size_bytes: 1024,
                ..Default::default()
            },
        );
        assert_eq!(filtered.len(), 1);
        assert!(filtered.contains_key("large.bin"));
    }

    #[test]
    fn walk_files_filtered_include_extensions() {
        let root = tempfile::tempdir().unwrap();
        write_temp_file(root.path(), "a.pdf", b"pdf");
        write_temp_file(root.path(), "b.txt", b"txt");
        write_temp_file(root.path(), "readme", b"no ext");

        let pdf_only = walk_files_filtered(
            root.path(),
            &[],
            WalkFilterOptions {
                extension_mode: CompareExtensionMode::Include,
                extensions: vec!["pdf".into()],
                ..Default::default()
            },
        );
        assert_eq!(pdf_only.len(), 1);
        assert!(pdf_only.contains_key("a.pdf"));
    }

    #[test]
    fn walk_files_filtered_exclude_extensions() {
        let root = tempfile::tempdir().unwrap();
        write_temp_file(root.path(), "a.pdf", b"pdf");
        write_temp_file(root.path(), "b.txt", b"txt");

        let no_txt = walk_files_filtered(
            root.path(),
            &[],
            WalkFilterOptions {
                extension_mode: CompareExtensionMode::Exclude,
                extensions: vec!["txt".into()],
                ..Default::default()
            },
        );
        assert_eq!(no_txt.len(), 1);
        assert!(no_txt.contains_key("a.pdf"));
    }
}
