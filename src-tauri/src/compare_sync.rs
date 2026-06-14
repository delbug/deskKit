use crate::fs_util::{copy_file_safe, resolve_safe_dir, walk_files, FsError};
use crate::models::{
    CompareExtensionMode, CompareMode, CompareResult, CompareStats, DiffEntry, FileMeta, FolderItem,
    FolderMeta, SyncParams, SyncPreviewOperation, SyncPreviewSummary, SyncStrategy,
};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

fn classify_entry(
    relative_path: &str,
    folder_maps: &HashMap<String, HashMap<String, FileMeta>>,
    folder_ids: &[String],
    mode: CompareMode,
    primary_id: &str,
) -> DiffEntry {
    let mut presence = HashMap::new();
    let mut sizes = HashMap::new();
    let mut md5s = HashMap::new();
    let mut mtimes = HashMap::new();

    for id in folder_ids {
        let info = folder_maps.get(id).and_then(|m| m.get(relative_path));
        presence.insert(id.clone(), info.is_some());
        sizes.insert(id.clone(), info.map(|i| i.size));
        md5s.insert(id.clone(), info.and_then(|i| i.md5.clone()));
        mtimes.insert(id.clone(), info.map(|i| i.mtime));
    }

    let present_ids: Vec<String> = folder_ids
        .iter()
        .filter(|id| *presence.get(*id).unwrap_or(&false))
        .cloned()
        .collect();
    let count = present_ids.len();

    let mut status = "identical".to_string();
    if count == 0 {
        status = "unknown".into();
    } else if count < folder_ids.len() {
        status = "missing".into();
    } else if mode == CompareMode::Md5 {
        let hashes: Vec<&String> = present_ids
            .iter()
            .filter_map(|id| md5s.get(id).and_then(|h| h.as_ref()))
            .collect();
        let unique: HashSet<_> = hashes.iter().copied().collect();
        status = if unique.len() <= 1 {
            "identical".into()
        } else {
            "content-diff".into()
        };
    }

    if count == 1 {
        status = format!("only-{}", present_ids[0]);
    }

    let primary_has = *presence.get(primary_id).unwrap_or(&false);
    let primary_only = primary_has
        && folder_ids
            .iter()
            .any(|id| id != primary_id && !*presence.get(id).unwrap_or(&false));
    let secondary_only = !primary_has && !present_ids.is_empty();

    DiffEntry {
        relative_path: relative_path.to_string(),
        status,
        presence,
        sizes,
        md5s,
        mtimes,
        present_count: count,
        primary_has,
        primary_only,
        secondary_only,
        present_ids,
        paths_by_folder: None,
        md5: None,
    }
}

fn detect_relocated(
    entries: Vec<DiffEntry>,
    folder_maps: &HashMap<String, HashMap<String, FileMeta>>,
    folder_ids: &[String],
    mode: CompareMode,
    primary_id: &str,
) -> (Vec<DiffEntry>, usize) {
    if mode != CompareMode::Md5 {
        return (entries, 0);
    }

    let mut by_md5: HashMap<String, Vec<(String, String, FileMeta)>> = HashMap::new();
    for id in folder_ids {
        if let Some(map) = folder_maps.get(id) {
            for (rel, info) in map {
                if let Some(hash) = &info.md5 {
                    by_md5
                        .entry(hash.clone())
                        .or_default()
                        .push((id.clone(), rel.clone(), info.clone()));
                }
            }
        }
    }

    let mut remove_keys = HashSet::new();
    let mut new_entries = Vec::new();

    for (hash, locations) in &by_md5 {
        let mut paths_by_folder: HashMap<String, String> = HashMap::new();
        let mut ambiguous = false;
        for (fid, rel, _) in locations {
            if let Some(existing) = paths_by_folder.get(fid) {
                if existing != rel {
                    ambiguous = true;
                    break;
                }
            }
            paths_by_folder.insert(fid.clone(), rel.clone());
        }
        if ambiguous {
            continue;
        }

        let valid_paths: Vec<(String, String)> = paths_by_folder.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        let unique_paths: std::collections::HashSet<_> = valid_paths.iter().map(|(_, p)| p.as_str()).collect();
        if valid_paths.len() < 2 || unique_paths.len() <= 1 {
            continue;
        }

        for (fid, p) in &paths_by_folder {
            remove_keys.insert(format!("{fid}:{p}"));
        }

        let display_path = paths_by_folder
            .get(primary_id)
            .cloned()
            .or_else(|| valid_paths.first().map(|(_, p)| p.clone()))
            .unwrap_or_default();

        let mut presence = HashMap::new();
        let mut sizes = HashMap::new();
        let mut md5s_map = HashMap::new();
        let mut mtimes = HashMap::new();

        for id in folder_ids {
            if let Some(p) = paths_by_folder.get(id) {
                if let Some(info) = folder_maps.get(id).and_then(|m| m.get(p)) {
                    presence.insert(id.clone(), true);
                    sizes.insert(id.clone(), Some(info.size));
                    md5s_map.insert(id.clone(), info.md5.clone());
                    mtimes.insert(id.clone(), Some(info.mtime));
                } else {
                    presence.insert(id.clone(), false);
                    sizes.insert(id.clone(), None);
                    md5s_map.insert(id.clone(), None);
                    mtimes.insert(id.clone(), None);
                }
            } else {
                presence.insert(id.clone(), false);
                sizes.insert(id.clone(), None);
                md5s_map.insert(id.clone(), None);
                mtimes.insert(id.clone(), None);
            }
        }

        let present_count = valid_paths.len();
        let primary_has = paths_by_folder.contains_key(primary_id);
        new_entries.push(DiffEntry {
            relative_path: display_path,
            status: "relocated".into(),
            paths_by_folder: Some(paths_by_folder),
            md5: Some(hash.clone()),
            presence,
            sizes,
            md5s: md5s_map,
            mtimes,
            present_count,
            primary_has,
            primary_only: false,
            secondary_only: false,
            present_ids: valid_paths.iter().map(|(fid, _)| fid.clone()).collect(),
        });
    }

    let filtered: Vec<DiffEntry> = entries
        .into_iter()
        .filter(|e| {
            for id in folder_ids {
                if *e.presence.get(id).unwrap_or(&false)
                    && remove_keys.contains(&format!("{id}:{}", e.relative_path))
                {
                    return false;
                }
            }
            true
        })
        .collect();

    let mut merged = filtered;
    merged.extend(new_entries);
    merged.sort_by(|a, b| {
        a.relative_path
            .cmp(&b.relative_path)
    });
    let relocated_count = merged.iter().filter(|e| e.status == "relocated").count();
    (merged, relocated_count)
}

pub async fn compare_folders(
    folders: Vec<FolderItem>,
    mode: CompareMode,
    ignore_patterns: Vec<String>,
    min_size_kb: Option<u64>,
    max_size_kb: Option<u64>,
    extension_mode: CompareExtensionMode,
    compare_extensions: Vec<String>,
) -> Result<CompareResult, FsError> {
    if folders.len() < 2 {
        return Err(FsError::Message("请至少添加 2 个文件夹".into()));
    }
    if extension_mode != CompareExtensionMode::None && compare_extensions.is_empty() {
        return Err(FsError::Message("请选择至少一种文件格式".into()));
    }
    if let (Some(min_kb), Some(max_kb)) = (min_size_kb, max_size_kb) {
        if min_kb > 0 && max_kb > 0 && min_kb > max_kb {
            return Err(FsError::Message("最小文件大小不能大于最大文件大小".into()));
        }
    }

    let mut folders = folders;
    if !folders.iter().any(|f| f.is_primary) {
        folders[0].is_primary = true;
    }

    let folder_ids: Vec<String> = folders.iter().map(|f| f.id.clone()).collect();
    let mut folder_maps: HashMap<String, HashMap<String, FileMeta>> = HashMap::new();
    let mut folder_meta: HashMap<String, FolderMeta> = HashMap::new();

    let walk_options = crate::fs_util::WalkFilterOptions {
        min_size_bytes: min_size_kb.unwrap_or(0).saturating_mul(1024),
        max_size_bytes: max_size_kb.unwrap_or(0).saturating_mul(1024),
        extension_mode,
        extensions: compare_extensions,
    };

    for folder in &folders {
        let root = resolve_safe_dir(&folder.path)?;
        let raw = crate::fs_util::walk_files_filtered(&root, &ignore_patterns, walk_options.clone());
        let file_count = raw.len();
        let label = if folder.label.is_empty() {
            root.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| folder.id.clone())
        } else {
            folder.label.clone()
        };
        folder_meta.insert(
            folder.id.clone(),
            FolderMeta {
                id: folder.id.clone(),
                path: root.to_string_lossy().into_owned(),
                label,
                is_primary: folder.is_primary,
                file_count,
            },
        );
        let mapped = if mode == CompareMode::Md5 {
            crate::fs_util::hash_files(raw).await
        } else {
            raw
        };
        folder_maps.insert(folder.id.clone(), mapped);
    }

    let primary_id = folders
        .iter()
        .find(|f| f.is_primary)
        .map(|f| f.id.clone())
        .unwrap_or_else(|| folders[0].id.clone());

    let mut all_paths = HashSet::new();
    for id in &folder_ids {
        if let Some(map) = folder_maps.get(id) {
            for rel in map.keys() {
                all_paths.insert(rel.clone());
            }
        }
    }

    let mut entry_list: Vec<DiffEntry> = all_paths
        .iter()
        .collect::<Vec<_>>()
        .into_iter()
        .map(|rel| classify_entry(rel, &folder_maps, &folder_ids, mode, &primary_id))
        .collect();
    entry_list.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    let (entry_list, relocated_count) =
        detect_relocated(entry_list, &folder_maps, &folder_ids, mode, &primary_id);

    let mut stats = CompareStats {
        total: 0,
        identical: 0,
        missing: 0,
        content_diff: 0,
        relocated: relocated_count,
        only_in: HashMap::new(),
    };
    for id in &folder_ids {
        stats.only_in.insert(id.clone(), 0);
    }

    for entry in &entry_list {
        stats.total += 1;
        match entry.status.as_str() {
            "identical" => stats.identical += 1,
            "missing" => stats.missing += 1,
            "content-diff" => stats.content_diff += 1,
            s if s.starts_with("only-") => {
                let fid = s.strip_prefix("only-").unwrap_or("");
                *stats.only_in.entry(fid.to_string()).or_insert(0) += 1;
            }
            _ => {}
        }
    }

    Ok(CompareResult {
        mode,
        primary_id,
        folders: folder_meta,
        entries: entry_list,
        stats,
    })
}

fn classify_file_action(src: &FileMeta, dst: Option<&FileMeta>) -> &'static str {
    match dst {
        None => "copy",
        Some(d) => {
            if src.size != d.size || (src.mtime - d.mtime).abs() > 1000.0 {
                "overwrite"
            } else {
                "skip"
            }
        }
    }
}

fn preview_primary_overwrite(
    primary_root: &Path,
    target_root: &Path,
    target_label: &str,
    relative_paths: Option<&[String]>,
    delete_extra: bool,
) -> Vec<SyncPreviewOperation> {
    let mut operations = Vec::new();
    let primary_files = walk_files(primary_root, &[]);
    let target_files = walk_files(target_root, &[]);

    let to_process: Vec<String> = match relative_paths {
        Some(paths) if !paths.is_empty() => paths
            .iter()
            .filter(|p| primary_files.contains_key(*p))
            .cloned()
            .collect(),
        _ => primary_files.keys().cloned().collect(),
    };

    for rel in &to_process {
        let src = primary_files.get(rel).unwrap();
        let dst = target_files.get(rel);
        let action = classify_file_action(src, dst);
        if action == "skip" {
            continue;
        }
        operations.push(SyncPreviewOperation {
            action: action.into(),
            relative_path: rel.clone(),
            target_label: Some(target_label.into()),
            detail: Some(if action == "copy" {
                "复制到目标".into()
            } else {
                "覆盖目标文件".into()
            }),
        });
    }

    let paths_provided = relative_paths.map(|p| !p.is_empty()).unwrap_or(false);
    if delete_extra && !paths_provided {
        for rel in target_files.keys() {
            if !primary_files.contains_key(rel) {
                operations.push(SyncPreviewOperation {
                    action: "delete".into(),
                    relative_path: rel.clone(),
                    target_label: Some(target_label.into()),
                    detail: Some("从目标删除（主文件夹无此文件）".into()),
                });
            }
        }
    } else if delete_extra {
        if let Some(paths) = relative_paths {
            for rel in paths {
                if target_files.contains_key(rel) && !primary_files.contains_key(rel) {
                    operations.push(SyncPreviewOperation {
                        action: "delete".into(),
                        relative_path: rel.clone(),
                        target_label: Some(target_label.into()),
                        detail: Some("从目标删除（主文件夹无此文件）".into()),
                    });
                }
            }
        }
    }

    operations
}

fn preview_union(
    folder_roots: &HashMap<String, PathBuf>,
    folder_labels: &HashMap<String, String>,
    primary_root: &Path,
    relative_paths: Option<&[String]>,
) -> Vec<SyncPreviewOperation> {
    let mut operations = Vec::new();
    let folder_ids: Vec<String> = folder_roots.keys().cloned().collect();
    let mut all_maps: HashMap<String, HashMap<String, FileMeta>> = HashMap::new();
    for (id, root) in folder_roots {
        all_maps.insert(id.clone(), walk_files(root, &[]));
    }

    let paths: Vec<String> = match relative_paths {
        Some(p) if !p.is_empty() => p.to_vec(),
        _ => folder_ids
            .iter()
            .flat_map(|id| all_maps.get(id).unwrap().keys().cloned())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect(),
    };

    let primary_key = folder_roots
        .iter()
        .find(|(_, r)| r.as_path() == primary_root)
        .map(|(id, _)| id.clone());

    for rel in paths {
        let holders: Vec<&String> = folder_ids
            .iter()
            .filter(|id| all_maps.get(*id).unwrap().contains_key(&rel))
            .collect();
        if holders.is_empty() || holders.len() == folder_ids.len() {
            continue;
        }

        let source_id = if let Some(ref pk) = primary_key {
            if holders.iter().any(|h| *h == pk) {
                pk.clone()
            } else {
                holders[0].clone()
            }
        } else {
            holders[0].clone()
        };

        for id in &folder_ids {
            if holders.iter().any(|h| *h == id) {
                continue;
            }
            operations.push(SyncPreviewOperation {
                action: "copy".into(),
                relative_path: rel.clone(),
                target_label: folder_labels.get(id).cloned(),
                detail: Some(format!(
                    "从 {} 复制缺失文件",
                    folder_labels.get(&source_id).cloned().unwrap_or_default()
                )),
            });
        }
    }
    operations
}

fn preview_selected(
    source_root: &Path,
    target_root: &Path,
    target_label: &str,
    relative_paths: &[String],
) -> Vec<SyncPreviewOperation> {
    let mut operations = Vec::new();
    let source_files = walk_files(source_root, &[]);
    let target_files = walk_files(target_root, &[]);

    for rel in relative_paths {
        if !source_files.contains_key(rel) {
            operations.push(SyncPreviewOperation {
                action: "skip".into(),
                relative_path: rel.clone(),
                target_label: Some(target_label.into()),
                detail: Some("源不存在，跳过".into()),
            });
            continue;
        }
        let action = classify_file_action(source_files.get(rel).unwrap(), target_files.get(rel));
        if action == "skip" {
            operations.push(SyncPreviewOperation {
                action: "skip".into(),
                relative_path: rel.clone(),
                target_label: Some(target_label.into()),
                detail: Some("已相同，跳过".into()),
            });
            continue;
        }
        operations.push(SyncPreviewOperation {
            action: action.into(),
            relative_path: rel.clone(),
            target_label: Some(target_label.into()),
            detail: Some(format!(
                "{} → {target_label}",
                if action == "copy" { "复制" } else { "覆盖" }
            )),
        });
    }
    operations
}

pub fn preview_sync(params: SyncParams) -> Result<(Vec<SyncPreviewOperation>, SyncPreviewSummary), FsError> {
    let primary = params
        .folders
        .iter()
        .find(|f| f.is_primary)
        .or_else(|| params.folders.first())
        .ok_or_else(|| FsError::Message("请至少 2 个文件夹".into()))?;
    let primary_root = resolve_safe_dir(&primary.path)?;
    let paths = if params.relative_paths.is_empty() {
        None
    } else {
        Some(params.relative_paths.as_slice())
    };

    let labels: HashMap<String, String> = params
        .folders
        .iter()
        .map(|f| {
            let label = if f.label.is_empty() {
                PathBuf::from(&f.path)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| f.id.clone())
            } else {
                f.label.clone()
            };
            (f.id.clone(), label)
        })
        .collect();

    let mut all_operations = Vec::new();

    match params.strategy {
        SyncStrategy::PrimaryOverwrite => {
            for sec in params.folders.iter().filter(|f| f.id != primary.id) {
                let target_root = resolve_safe_dir(&sec.path)?;
                all_operations.extend(preview_primary_overwrite(
                    &primary_root,
                    &target_root,
                    labels.get(&sec.id).map(String::as_str).unwrap_or(""),
                    paths,
                    params.delete_extra,
                ));
            }
        }
        SyncStrategy::Union => {
            let folder_roots: HashMap<String, PathBuf> = params
                .folders
                .iter()
                .map(|f| {
                    (
                        f.id.clone(),
                        resolve_safe_dir(&f.path).unwrap_or_else(|_| PathBuf::from(&f.path)),
                    )
                })
                .collect();
            all_operations.extend(preview_union(
                &folder_roots,
                &labels,
                &primary_root,
                paths,
            ));
        }
        SyncStrategy::Selected => {
            let src = params
                .source_folder_id
                .as_ref()
                .and_then(|id| params.folders.iter().find(|f| &f.id == id))
                .ok_or_else(|| FsError::Message("文件夹 ID 无效".into()))?;
            let tgt = params
                .target_folder_id
                .as_ref()
                .and_then(|id| params.folders.iter().find(|f| &f.id == id))
                .ok_or_else(|| FsError::Message("文件夹 ID 无效".into()))?;
            let source_root = resolve_safe_dir(&src.path)?;
            let target_root = resolve_safe_dir(&tgt.path)?;
            all_operations.extend(preview_selected(
                &source_root,
                &target_root,
                labels.get(&tgt.id).map(String::as_str).unwrap_or(""),
                paths.unwrap_or(&[]),
            ));
        }
    }

    let summary = SyncPreviewSummary {
        copy: all_operations.iter().filter(|o| o.action == "copy").count(),
        overwrite: all_operations.iter().filter(|o| o.action == "overwrite").count(),
        delete: all_operations.iter().filter(|o| o.action == "delete").count(),
        skip: all_operations.iter().filter(|o| o.action == "skip").count(),
        total: all_operations.iter().filter(|o| o.action != "skip").count(),
    };

    Ok((all_operations, summary))
}

fn sync_primary_overwrite(
    primary_root: &Path,
    target_root: &Path,
    relative_paths: Option<&[String]>,
    delete_extra: bool,
) -> serde_json::Value {
    let mut copied = Vec::new();
    let mut deleted = Vec::new();
    let mut errors = Vec::new();

    let primary_files = walk_files(primary_root, &[]);
    let target_files = walk_files(target_root, &[]);

    let to_copy: Vec<String> = match relative_paths {
        Some(paths) => paths
            .iter()
            .filter(|p| primary_files.contains_key(*p))
            .cloned()
            .collect(),
        None => primary_files.keys().cloned().collect(),
    };

    for rel in &to_copy {
        let src = PathBuf::from(&primary_files.get(rel).unwrap().absolute_path);
        let dst = target_root.join(rel);
        match copy_file_safe(&src, &dst) {
            Ok(()) => copied.push(rel.clone()),
            Err(e) => errors.push(serde_json::json!({ "path": rel, "message": e.to_string() })),
        }
    }

    let paths_provided = relative_paths.map(|p| !p.is_empty()).unwrap_or(false);
    if delete_extra && !paths_provided {
        for rel in target_files.keys() {
            if !primary_files.contains_key(rel) {
                let full = target_root.join(rel);
                match fs::remove_file(&full) {
                    Ok(()) => deleted.push(rel.clone()),
                    Err(e) => errors.push(serde_json::json!({
                        "path": rel,
                        "message": format!("删除失败: {e}")
                    })),
                }
            }
        }
    } else if delete_extra {
        if let Some(paths) = relative_paths {
            for rel in paths {
                if target_files.contains_key(rel) && !primary_files.contains_key(rel) {
                    let full = target_root.join(rel);
                    match fs::remove_file(&full) {
                        Ok(()) => deleted.push(rel.clone()),
                        Err(e) => errors.push(serde_json::json!({
                            "path": rel,
                            "message": format!("删除失败: {e}")
                        })),
                    }
                }
            }
        }
    }

    serde_json::json!({ "copied": copied, "deleted": deleted, "errors": errors })
}

fn sync_union(
    folder_roots: &HashMap<String, PathBuf>,
    primary_root: &Path,
    relative_paths: Option<&[String]>,
) -> serde_json::Value {
    let mut copied = Vec::new();
    let mut errors = Vec::new();
    let folder_ids: Vec<String> = folder_roots.keys().cloned().collect();
    let mut all_maps: HashMap<String, HashMap<String, FileMeta>> = HashMap::new();
    for (id, root) in folder_roots {
        all_maps.insert(id.clone(), walk_files(root, &[]));
    }

    let paths: Vec<String> = match relative_paths {
        Some(p) if !p.is_empty() => p.to_vec(),
        _ => folder_ids
            .iter()
            .flat_map(|id| all_maps.get(id).unwrap().keys().cloned())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect(),
    };

    let primary_key = folder_roots
        .iter()
        .find(|(_, r)| r.as_path() == primary_root)
        .map(|(id, _)| id.clone());

    for rel in paths {
        let holders: Vec<String> = folder_ids
            .iter()
            .filter(|id| all_maps.get(*id).unwrap().contains_key(&rel))
            .cloned()
            .collect();
        if holders.is_empty() {
            continue;
        }

        let source_id = if let Some(ref pk) = primary_key {
            if holders.contains(pk) {
                pk.clone()
            } else if holders.len() > 1 {
                holders
                    .iter()
                    .reduce(|best, id| {
                        let m = all_maps.get(id).unwrap().get(&rel).map(|f| f.mtime).unwrap_or(0.0);
                        let bm = all_maps.get(best).unwrap().get(&rel).map(|f| f.mtime).unwrap_or(0.0);
                        if m > bm { id } else { best }
                    })
                    .cloned()
                    .unwrap_or_else(|| holders[0].clone())
            } else {
                holders[0].clone()
            }
        } else {
            holders[0].clone()
        };

        let src = PathBuf::from(
            all_maps
                .get(&source_id)
                .unwrap()
                .get(&rel)
                .unwrap()
                .absolute_path
                .clone(),
        );

        for id in &folder_ids {
            if id == &source_id || all_maps.get(id).unwrap().contains_key(&rel) {
                continue;
            }
            let dst = folder_roots.get(id).unwrap().join(&rel);
            match copy_file_safe(&src, &dst) {
                Ok(()) => copied.push(serde_json::json!({
                    "path": rel,
                    "to": folder_roots.get(id).unwrap().to_string_lossy(),
                    "from": folder_roots.get(&source_id).unwrap().to_string_lossy(),
                })),
                Err(e) => errors.push(serde_json::json!({ "path": rel, "message": e.to_string() })),
            }
        }
    }

    serde_json::json!({ "copied": copied, "errors": errors })
}

fn sync_selected(
    source_root: &Path,
    target_root: &Path,
    relative_paths: &[String],
) -> serde_json::Value {
    let mut copied = Vec::new();
    let mut skipped = Vec::new();
    let mut errors = Vec::new();

    for rel in relative_paths {
        let src = source_root.join(rel);
        let dst = target_root.join(rel);
        if !src.exists() {
            skipped.push(serde_json::json!({ "path": rel, "reason": "源不存在" }));
            continue;
        }
        match copy_file_safe(&src, &dst) {
            Ok(()) => copied.push(rel.clone()),
            Err(e) => errors.push(serde_json::json!({ "path": rel, "message": e.to_string() })),
        }
    }

    serde_json::json!({ "copied": copied, "skipped": skipped, "errors": errors })
}

pub fn sync_folders(params: SyncParams) -> Result<Vec<serde_json::Value>, FsError> {
    if params.folders.len() < 2 {
        return Err(FsError::Message("请至少 2 个文件夹".into()));
    }

    let primary = params
        .folders
        .iter()
        .find(|f| f.is_primary)
        .or_else(|| params.folders.first())
        .unwrap();
    let primary_root = resolve_safe_dir(&primary.path)?;
    let paths = if params.relative_paths.is_empty() {
        None
    } else {
        Some(params.relative_paths.as_slice())
    };
    let mut results = Vec::new();

    match params.strategy {
        SyncStrategy::PrimaryOverwrite => {
            for sec in params.folders.iter().filter(|f| f.id != primary.id) {
                let target_root = resolve_safe_dir(&sec.path)?;
                let mut result = sync_primary_overwrite(
                    &primary_root,
                    &target_root,
                    paths,
                    params.delete_extra,
                );
                if let Some(obj) = result.as_object_mut() {
                    obj.insert("targetId".into(), sec.id.clone().into());
                    obj.insert(
                        "targetPath".into(),
                        target_root.to_string_lossy().into_owned().into(),
                    );
                }
                results.push(result);
            }
        }
        SyncStrategy::Union => {
            let folder_roots: HashMap<String, PathBuf> = params
                .folders
                .iter()
                .map(|f| (f.id.clone(), resolve_safe_dir(&f.path).unwrap()))
                .collect();
            let mut result = sync_union(&folder_roots, &primary_root, paths);
            if let Some(obj) = result.as_object_mut() {
                obj.insert("strategy".into(), "union".into());
            }
            results.push(result);
        }
        SyncStrategy::Selected => {
            let source_id = params
                .source_folder_id
                .clone()
                .ok_or_else(|| FsError::Message("请指定源和目标文件夹".into()))?;
            let target_id = params
                .target_folder_id
                .clone()
                .ok_or_else(|| FsError::Message("请指定源和目标文件夹".into()))?;
            let src = params
                .folders
                .iter()
                .find(|f| f.id == source_id)
                .ok_or_else(|| FsError::Message("文件夹 ID 无效".into()))?;
            let tgt = params
                .folders
                .iter()
                .find(|f| f.id == target_id)
                .ok_or_else(|| FsError::Message("文件夹 ID 无效".into()))?;
            let source_root = resolve_safe_dir(&src.path)?;
            let target_root = resolve_safe_dir(&tgt.path)?;
            let mut result = sync_selected(&source_root, &target_root, paths.unwrap_or(&[]));
            if let Some(obj) = result.as_object_mut() {
                obj.insert("sourceId".into(), source_id.into());
                obj.insert("targetId".into(), target_id.into());
            }
            results.push(result);
        }
    }

    Ok(results)
}

pub fn delete_files(
    items: Vec<crate::models::DeleteFileItem>,
) -> Result<(Vec<serde_json::Value>, Vec<serde_json::Value>), FsError> {
    let mut deleted = Vec::new();
    let mut errors = Vec::new();
    for item in items {
        match resolve_safe_dir(&item.folder_path) {
            Ok(root) => {
                let full = root.join(&item.relative_path);
                if !full.exists() {
                    errors.push(serde_json::json!({
                        "folderPath": item.folder_path,
                        "relativePath": item.relative_path,
                        "message": "文件不存在"
                    }));
                } else {
                    match fs::remove_file(&full) {
                        Ok(()) => deleted.push(serde_json::to_value(&item).unwrap()),
                        Err(e) => errors.push(serde_json::json!({
                            "folderPath": item.folder_path,
                            "relativePath": item.relative_path,
                            "message": e.to_string()
                        })),
                    }
                }
            }
            Err(e) => errors.push(serde_json::json!({
                "folderPath": item.folder_path,
                "relativePath": item.relative_path,
                "message": e.to_string()
            })),
        }
    }
    Ok((deleted, errors))
}

pub fn move_files(
    items: Vec<crate::models::MoveFileItem>,
) -> Result<(Vec<serde_json::Value>, Vec<serde_json::Value>), FsError> {
    let mut moved = Vec::new();
    let mut errors = Vec::new();
    for item in items {
        let result: Result<(), FsError> = (|| {
            let from_root = resolve_safe_dir(&item.from_folder_path)?;
            let to_root = resolve_safe_dir(&item.to_folder_path)?;
            let src = from_root.join(&item.relative_path);
            let target_rel = item
                .target_relative_path
                .as_deref()
                .unwrap_or(&item.relative_path);
            let dst = to_root.join(target_rel);
            if !src.exists() {
                return Err(FsError::Message(format!("源文件不存在: {}", item.relative_path)));
            }
            crate::fs_util::ensure_dir_for_file(&dst)?;
            fs::rename(&src, &dst)?;
            Ok(())
        })();
        match result {
            Ok(()) => moved.push(serde_json::to_value(&item).unwrap()),
            Err(e) => errors.push(serde_json::json!({
                "fromFolderPath": item.from_folder_path,
                "toFolderPath": item.to_folder_path,
                "relativePath": item.relative_path,
                "message": e.to_string()
            })),
        }
    }
    Ok((moved, errors))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CompareMode, FolderItem};
    use std::io::Write;

    fn write_file(dir: &Path, name: &str, bytes: &[u8]) {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(bytes).unwrap();
    }

    #[tokio::test]
    async fn compare_folders_excludes_files_below_min_size_kb() {
        let base = tempfile::tempdir().unwrap();
        let dir_a = base.path().join("a");
        let dir_b = base.path().join("b");
        std::fs::create_dir_all(&dir_a).unwrap();
        std::fs::create_dir_all(&dir_b).unwrap();

        write_file(&dir_a, "tiny.txt", b"x");
        write_file(&dir_a, "shared.bin", &vec![1u8; 2048]);
        write_file(&dir_b, "shared.bin", &vec![1u8; 2048]);

        let folders = vec![
            FolderItem {
                id: "a".into(),
                path: dir_a.to_string_lossy().into_owned(),
                label: String::new(),
                is_primary: true,
            },
            FolderItem {
                id: "b".into(),
                path: dir_b.to_string_lossy().into_owned(),
                label: String::new(),
                is_primary: false,
            },
        ];

        let with_filter = compare_folders(
            folders.clone(),
            CompareMode::Name,
            vec![],
            Some(1),
            None,
            CompareExtensionMode::None,
            vec![],
        )
            .await
            .unwrap();
        assert_eq!(with_filter.stats.identical, 1);
        assert!(!with_filter.entries.iter().any(|e| e.relative_path == "tiny.txt"));

        let without_filter = compare_folders(
            folders,
            CompareMode::Name,
            vec![],
            None,
            None,
            CompareExtensionMode::None,
            vec![],
        )
            .await
            .unwrap();
        assert!(without_filter.entries.iter().any(|e| e.relative_path == "tiny.txt"));
    }

    #[tokio::test]
    async fn compare_folders_include_extension_only() {
        let base = tempfile::tempdir().unwrap();
        let dir_a = base.path().join("a");
        let dir_b = base.path().join("b");
        std::fs::create_dir_all(&dir_a).unwrap();
        std::fs::create_dir_all(&dir_b).unwrap();

        write_file(&dir_a, "doc.pdf", b"pdf");
        write_file(&dir_a, "note.txt", b"txt");
        write_file(&dir_b, "doc.pdf", b"pdf");
        write_file(&dir_b, "note.txt", b"txt");

        let folders = vec![
            FolderItem {
                id: "a".into(),
                path: dir_a.to_string_lossy().into_owned(),
                label: String::new(),
                is_primary: true,
            },
            FolderItem {
                id: "b".into(),
                path: dir_b.to_string_lossy().into_owned(),
                label: String::new(),
                is_primary: false,
            },
        ];

        let result = compare_folders(
            folders,
            CompareMode::Name,
            vec![],
            None,
            None,
            CompareExtensionMode::Include,
            vec!["pdf".into()],
        )
        .await
        .unwrap();

        assert_eq!(result.stats.total, 1);
        assert_eq!(result.entries[0].relative_path, "doc.pdf");
        assert_eq!(result.stats.identical, 1);
    }

    #[tokio::test]
    async fn compare_folders_excludes_files_above_max_size_kb() {
        let base = tempfile::tempdir().unwrap();
        let dir_a = base.path().join("a");
        let dir_b = base.path().join("b");
        std::fs::create_dir_all(&dir_a).unwrap();
        std::fs::create_dir_all(&dir_b).unwrap();

        write_file(&dir_a, "tiny.txt", b"x");
        write_file(&dir_a, "large.bin", &vec![0u8; 2048]);
        write_file(&dir_b, "tiny.txt", b"x");

        let folders = vec![
            FolderItem {
                id: "a".into(),
                path: dir_a.to_string_lossy().into_owned(),
                label: String::new(),
                is_primary: true,
            },
            FolderItem {
                id: "b".into(),
                path: dir_b.to_string_lossy().into_owned(),
                label: String::new(),
                is_primary: false,
            },
        ];

        let result = compare_folders(
            folders,
            CompareMode::Name,
            vec![],
            None,
            Some(1),
            CompareExtensionMode::None,
            vec![],
        )
        .await
        .unwrap();

        assert_eq!(result.stats.total, 1);
        assert_eq!(result.entries[0].relative_path, "tiny.txt");
        assert_eq!(result.stats.identical, 1);
    }
}
