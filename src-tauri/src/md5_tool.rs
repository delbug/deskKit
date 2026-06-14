use crate::fs_util::{file_meta_for_path, hash_files, md5_file, resolve_safe_dir, walk_files_filtered, WalkFilterOptions, FsError};
use crate::models::{FileMeta, Md5RandomizeDetail, Md5RandomizeResult, Md5RenameItem, Md5RenameMode, Md5RenameResult, Md5ScanResult, Md5ScanStats, Md5VerifyResult};
use rand::Rng;
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

fn min_size_bytes(min_size_kb: Option<u64>) -> u64 {
    min_size_kb.unwrap_or(0).saturating_mul(1024)
}

pub async fn scan_md5(
    path: &str,
    min_size_kb: Option<u64>,
    ignore_patterns: Vec<String>,
) -> Result<Md5ScanResult, FsError> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(FsError::Message("路径不能为空".into()));
    }
    let input = PathBuf::from(trimmed);
    if !input.exists() {
        return Err(FsError::Message(format!("路径不存在: {trimmed}")));
    }

    let min_bytes = min_size_bytes(min_size_kb);
    let mut file_map: HashMap<String, FileMeta> = HashMap::new();
    let root_path;
    let is_file;

    if input.is_file() {
        let meta = file_meta_for_path(&input)?;
        if min_bytes > 0 && meta.size < min_bytes {
            return Err(FsError::Message(format!(
                "文件小于最小大小 {} KB",
                min_size_kb.unwrap_or(0)
            )));
        }
        root_path = meta.absolute_path.clone();
        is_file = true;
        file_map.insert(meta.relative_path.clone(), meta);
    } else {
        let root = resolve_safe_dir(trimmed)?;
        root_path = root.to_string_lossy().into_owned();
        is_file = false;
        file_map = walk_files_filtered(
            &root,
            &ignore_patterns,
            WalkFilterOptions {
                min_size_bytes: min_bytes,
                ..Default::default()
            },
        );
    }

    let hashed = hash_files(file_map).await;
    let mut entries: Vec<FileMeta> = hashed.into_values().collect();
    entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    let mut errors = 0usize;
    for e in &entries {
        if e.md5.is_none() {
            errors += 1;
        }
    }
    let total = entries.len();

    Ok(Md5ScanResult {
        root_path,
        is_file,
        entries,
        stats: Md5ScanStats { total, errors },
    })
}

pub fn md5_manifest_csv(entries: &[FileMeta]) -> String {
    let mut lines = vec!["relative_path,size,md5".to_string()];
    for e in entries {
        let md5 = e.md5.as_deref().unwrap_or("");
        lines.push(format!(
            "{},{},{}",
            csv_escape(&e.relative_path),
            e.size,
            md5
        ));
    }
    lines.join("\n")
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn target_name_for_mode(mode: Md5RenameMode, md5: &str, relative_path: &str) -> Result<String, String> {
    let path = Path::new(relative_path);
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("无效文件名: {relative_path}"))?;
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(file_name);
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();

    let name = match mode {
        Md5RenameMode::Prefix => format!("{md5}_{file_name}"),
        Md5RenameMode::Suffix => format!("{stem}_{md5}{ext}"),
        Md5RenameMode::HashOnly => format!("{md5}{ext}"),
    };
    Ok(crate::fs_util::safe_file_name(&name))
}

pub fn batch_rename_by_md5(
    root_path: &str,
    entries: Vec<Md5RenameItem>,
    mode: Md5RenameMode,
    dry_run: bool,
) -> Result<Md5RenameResult, FsError> {
    let root = PathBuf::from(root_path.trim());
    if !root.exists() {
        return Err(FsError::Message(format!("根路径不存在: {root_path}")));
    }
    let root_is_file = root.is_file();
    let root_dir = if root_is_file {
        root.parent()
            .ok_or_else(|| FsError::Message("无法解析文件所在目录".into()))?
            .to_path_buf()
    } else {
        resolve_safe_dir(root_path)?
    };

    let mut renamed = 0usize;
    let mut skipped = 0usize;
    let mut errors: Vec<String> = Vec::new();
    let mut used_names: HashMap<String, usize> = HashMap::new();

    for item in entries {
        let md5 = match item.md5.as_deref() {
            Some(h) if !h.is_empty() => h,
            _ => {
                skipped += 1;
                continue;
            }
        };
        let src = if root_is_file {
            root.clone()
        } else {
            root_dir.join(&item.relative_path)
        };
        if !src.is_file() {
            errors.push(format!("文件不存在: {}", item.relative_path));
            continue;
        }
        let parent = src.parent().unwrap_or(&root_dir);
        let mut new_name = target_name_for_mode(mode, md5, &item.relative_path)
            .map_err(FsError::Message)?;
        if let Some(count) = used_names.get(&new_name).copied() {
            let path = Path::new(&new_name);
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(&new_name);
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .map(|e| format!(".{e}"))
                .unwrap_or_default();
            new_name = format!("{stem}_{}{ext}", count + 2);
        }
        used_names.insert(new_name.clone(), 1);
        let dst = parent.join(&new_name);
        if src == dst {
            skipped += 1;
            continue;
        }
        if dst.exists() {
            errors.push(format!("目标已存在: {}", dst.display()));
            continue;
        }
        if !dry_run {
            fs::rename(&src, &dst).map_err(|e| FsError::Io(e))?;
        }
        renamed += 1;
    }

    Ok(Md5RenameResult {
        renamed,
        skipped,
        dry_run,
        errors,
    })
}

fn resolve_file_path(root: &Path, root_is_file: bool, root_dir: &Path, relative_path: &str) -> PathBuf {
    if root_is_file {
        root.to_path_buf()
    } else {
        root_dir.join(relative_path)
    }
}

pub fn batch_randomize_md5(
    root_path: &str,
    relative_paths: Vec<String>,
    dry_run: bool,
) -> Result<Md5RandomizeResult, FsError> {
    let root = PathBuf::from(root_path.trim());
    if !root.exists() {
        return Err(FsError::Message(format!("根路径不存在: {root_path}")));
    }
    let root_is_file = root.is_file();
    let root_dir = if root_is_file {
        root.parent()
            .ok_or_else(|| FsError::Message("无法解析文件所在目录".into()))?
            .to_path_buf()
    } else {
        resolve_safe_dir(root_path)?
    };

    let paths: Vec<String> = if root_is_file {
        vec![root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string()]
    } else if relative_paths.is_empty() {
        return Err(FsError::Message("请选择要修改的文件".into()));
    } else {
        relative_paths
    };

    let mut modified = 0usize;
    let skipped = 0usize;
    let mut errors: Vec<String> = Vec::new();
    let mut details: Vec<Md5RandomizeDetail> = Vec::new();

    for relative_path in paths {
        let file_path = resolve_file_path(&root, root_is_file, &root_dir, &relative_path);
        if !file_path.is_file() {
            errors.push(format!("文件不存在: {relative_path}"));
            continue;
        }
        let old_md5 = match md5_file(&file_path) {
            Ok(h) => Some(h),
            Err(e) => {
                errors.push(format!("无法读取 {relative_path}: {e}"));
                continue;
            }
        };
        if dry_run {
            modified += 1;
            details.push(Md5RandomizeDetail {
                relative_path,
                old_md5,
                new_md5: None,
            });
            continue;
        }
        let mut suffix = [0u8; 32];
        rand::thread_rng().fill(&mut suffix);
        OpenOptions::new()
            .append(true)
            .open(&file_path)
            .and_then(|mut f| f.write_all(&suffix))
            .map_err(FsError::Io)?;
        let new_md5 = md5_file(&file_path).map_err(FsError::Io)?;
        modified += 1;
        details.push(Md5RandomizeDetail {
            relative_path,
            old_md5,
            new_md5: Some(new_md5),
        });
    }

    Ok(Md5RandomizeResult {
        modified,
        skipped,
        dry_run,
        errors,
        details,
    })
}

fn split_csv_line(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quote = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '"' {
            if in_quote && chars.peek() == Some(&'"') {
                chars.next();
                cur.push('"');
            } else {
                in_quote = !in_quote;
            }
        } else if c == ',' && !in_quote {
            out.push(cur.trim().to_string());
            cur.clear();
        } else {
            cur.push(c);
        }
    }
    out.push(cur.trim().to_string());
    out
}

fn is_md5_hash(s: &str) -> bool {
    s.len() == 32 && s.chars().all(|c| c.is_ascii_hexdigit())
}

fn parse_manifest_line(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') || line.starts_with("relative_path") {
        return None;
    }
    if !line.contains(',') {
        let (hash, path) = line.split_once(' ')?;
        let hash = hash.trim().trim_matches('"');
        let path = path.trim().trim_matches('"');
        return is_md5_hash(hash).then(|| (path.to_string(), hash.to_ascii_lowercase()));
    }

    let fields = split_csv_line(line);
    let md5_idx = fields.iter().position(|f| is_md5_hash(f))?;
    let md5 = fields[md5_idx].to_ascii_lowercase();
    let path = if md5_idx == 0 && fields.len() > 1 {
        fields[1].clone()
    } else {
        fields[0].clone()
    };
    Some((path, md5))
}

pub fn verify_md5_manifest(
    entries: &[FileMeta],
    manifest_text: &str,
) -> Md5VerifyResult {
    let mut expected: HashMap<String, String> = HashMap::new();
    for line in manifest_text.lines() {
        if let Some((path, md5)) = parse_manifest_line(line) {
            expected.insert(path, md5);
        }
    }

    let mut matched = 0usize;
    let mut mismatched = 0usize;
    let mut missing = 0usize;
    let mut details: Vec<serde_json::Value> = Vec::new();

    for (path, exp) in &expected {
        match entries.iter().find(|e| e.relative_path == *path) {
            Some(entry) => {
                let actual = entry.md5.as_deref().unwrap_or("").to_ascii_lowercase();
                if actual == *exp {
                    matched += 1;
                } else {
                    mismatched += 1;
                    details.push(serde_json::json!({
                        "path": path,
                        "expected": exp,
                        "actual": entry.md5,
                        "status": "mismatch",
                    }));
                }
            }
            None => {
                missing += 1;
                details.push(serde_json::json!({
                    "path": path,
                    "expected": exp,
                    "status": "missing",
                }));
            }
        }
    }

    Md5VerifyResult {
        matched,
        mismatched,
        missing,
        total: expected.len(),
        details,
    }
}

pub fn md5_of_path(path: &str) -> Result<String, FsError> {
    let p = PathBuf::from(path.trim());
    if !p.is_file() {
        return Err(FsError::Message("请指定存在的文件路径".into()));
    }
    md5_file(&p.canonicalize().map_err(FsError::Io)?).map_err(FsError::Io)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Md5RenameItem, Md5RenameMode};
    use std::io::Write;

    fn write_file(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(bytes).unwrap();
        path
    }

    #[tokio::test]
    async fn scan_md5_respects_min_size_kb() {
        let root = tempfile::tempdir().unwrap();
        write_file(root.path(), "small.txt", b"a");
        write_file(root.path(), "big.bin", &vec![0u8; 2048]);

        let result = scan_md5(root.path().to_str().unwrap(), Some(1), vec![])
            .await
            .unwrap();
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].relative_path, "big.bin");
        assert!(result.entries[0].md5.is_some());
    }

    #[tokio::test]
    async fn scan_md5_single_file() {
        let root = tempfile::tempdir().unwrap();
        let file = write_file(root.path(), "hello.txt", b"hello");
        let expected = md5_file(&file).unwrap();

        let result = scan_md5(file.to_str().unwrap(), None, vec![])
            .await
            .unwrap();
        assert!(result.is_file);
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].md5.as_deref(), Some(expected.as_str()));
    }

    #[test]
    fn md5_manifest_csv_format() {
        let entries = vec![FileMeta {
            relative_path: "a,b.txt".into(),
            absolute_path: "/tmp/a,b.txt".into(),
            size: 10,
            mtime: 0.0,
            md5: Some("abc123".into()),
            error: None,
        }];
        let csv = md5_manifest_csv(&entries);
        assert!(csv.starts_with("relative_path,size,md5\n"));
        assert!(csv.contains("\"a,b.txt\""));
    }

    #[test]
    fn verify_md5_manifest_matches_and_mismatches() {
        let hash = "0123456789abcdef0123456789abcdef";
        let entries = vec![FileMeta {
            relative_path: "ok.txt".into(),
            absolute_path: String::new(),
            size: 1,
            mtime: 0.0,
            md5: Some(hash.into()),
            error: None,
        }];
        let manifest = format!("ok.txt,{hash}\nmissing.txt,abcdef0123456789abcdef0123456789\n");
        let result = verify_md5_manifest(&entries, &manifest);
        assert_eq!(result.matched, 1);
        assert_eq!(result.missing, 1);
        assert_eq!(result.mismatched, 0);
    }

    #[test]
    fn verify_md5_manifest_parses_exported_csv() {
        let hash = "0123456789abcdef0123456789abcdef";
        let entries = vec![FileMeta {
            relative_path: "a,b.txt".into(),
            absolute_path: String::new(),
            size: 10,
            mtime: 0.0,
            md5: Some(hash.into()),
            error: None,
        }];
        let csv = format!("relative_path,size,md5\n\"a,b.txt\",10,{hash}\n");
        let result = verify_md5_manifest(&entries, &csv);
        assert_eq!(result.matched, 1);
        assert_eq!(result.total, 1);
    }

    #[test]
    fn batch_rename_by_md5_dry_run_suffix() {
        let root = tempfile::tempdir().unwrap();
        let file = write_file(root.path(), "photo.jpg", b"image-bytes");
        let hash = md5_file(&file).unwrap();

        let result = batch_rename_by_md5(
            root.path().to_str().unwrap(),
            vec![Md5RenameItem {
                relative_path: "photo.jpg".into(),
                md5: Some(hash.clone()),
            }],
            Md5RenameMode::Suffix,
            true,
        )
        .unwrap();

        assert_eq!(result.renamed, 1);
        assert!(file.exists(), "dry run must not rename");
        let expected_stem = format!("photo_{hash}");
        assert!(Path::new(&expected_stem).file_stem().unwrap().to_str().unwrap().starts_with("photo_"));
    }

    #[test]
    fn batch_randomize_md5_changes_file_hash() {
        let root = tempfile::tempdir().unwrap();
        let file = write_file(root.path(), "a.txt", b"hello");
        let old = md5_file(&file).unwrap();

        let result = batch_randomize_md5(root.path().to_str().unwrap(), vec!["a.txt".into()], false).unwrap();
        assert_eq!(result.modified, 1);
        let new = md5_file(&file).unwrap();
        assert_ne!(old, new);
        assert_eq!(result.details[0].old_md5.as_deref(), Some(old.as_str()));
        assert_eq!(result.details[0].new_md5.as_deref(), Some(new.as_str()));
    }
}
