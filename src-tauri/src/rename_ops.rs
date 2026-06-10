use crate::models::{RenamePlanItem, RenameRules, RenameStats};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Clone)]
struct RenameEntry {
    relative_path: String,
    name: String,
    kind: String,
    parent: String,
    absolute_path: String,
}

pub fn sanitize_name(name: &str) -> String {
    let re = Regex::new(r#"[<>:"/\\|?*\x00-\x1f]"#).unwrap();
    let mut result = re.replace_all(name.trim(), "_").to_string();
    while result.ends_with('.') {
        result.pop();
    }
    if result.len() > 255 {
        result.truncate(255);
    }
    result
}

fn split_base_ext(filename: &str) -> (String, String) {
    if let Some(pos) = filename.rfind('.') {
        if pos > 0 {
            return (filename[..pos].to_string(), filename[pos..].to_string());
        }
    }
    (filename.to_string(), String::new())
}

fn apply_remove(base: &str, patterns: &[String]) -> String {
    let mut n = base.to_string();
    for pat in patterns {
        if pat.is_empty() {
            continue;
        }
        if let Ok(re) = Regex::new(pat) {
            n = re.replace_all(&n, "").to_string();
        } else {
            n = n.replace(pat, "");
        }
    }
    n
}

fn apply_replace(base: &str, from: &str, to: &str) -> String {
    if from.is_empty() {
        return base.to_string();
    }
    base.replace(from, to)
}

fn format_sequence(num: i32, pad_width: i32) -> String {
    let s = num.to_string();
    if pad_width <= 0 {
        return s;
    }
    format!("{:0>width$}", s, width = pad_width as usize)
}

fn apply_delete_at(base: &str, start: i32, count: i32) -> String {
    let n = count.max(0) as usize;
    if n == 0 {
        return base.to_string();
    }
    let keep = start.max(0) as usize;
    if keep >= base.len() {
        return base.to_string();
    }
    let end = (keep + n).min(base.len());
    format!("{}{}", &base[..keep], &base[end..])
}

fn apply_insert(base: &str, index: i32, content: &str) -> String {
    let i = index.max(0) as usize;
    let i = i.min(base.len());
    format!("{}{}{}", &base[..i], content, &base[i..])
}

pub fn transform_file_name(
    original_name: &str,
    rules: &RenameRules,
    seq_index: usize,
) -> (String, Option<String>) {
    let (base, ext) = split_base_ext(original_name);

    if rules.include_extension {
        let name = sanitize_name(original_name);
        if name.is_empty() {
            return (String::new(), Some("处理后名称为空".into()));
        }
        return (name, None);
    }

    let mut name = base;

    if let Some(ref patterns) = rules.remove_patterns {
        name = apply_remove(&name, patterns);
    }

    if let Some(ref del) = rules.delete_at {
        if del.enabled {
            name = apply_delete_at(&name, del.start, del.count);
        }
    }

    if let Some(ref from) = rules.replace_from {
        let to = rules.replace_to.as_deref().unwrap_or("");
        name = apply_replace(&name, from, to);
    }

    if let Some(ref replacements) = rules.replacements {
        for pair in replacements {
            if !pair.from.is_empty() {
                name = apply_replace(&name, &pair.from, pair.to.as_deref().unwrap_or(""));
            }
        }
    }

    if let Some(ref insert) = rules.insert {
        if insert.enabled {
            let insert_content = if insert.use_sequence {
                let seq = rules.sequence.as_ref();
                let start = seq.map(|s| s.start).unwrap_or(1);
                let step = seq.map(|s| s.step).unwrap_or(1);
                let pad = seq.map(|s| s.pad_width).unwrap_or(0);
                format_sequence(start + (seq_index as i32) * step, pad)
            } else {
                insert.content.clone().unwrap_or_default()
            };
            if !insert_content.is_empty() {
                name = apply_insert(&name, insert.index, &insert_content);
            }
        }
    }

    if let Some(ref seq) = rules.sequence {
        if seq.enabled && seq.position == "insert" {
            let num = format_sequence(
                seq.start + (seq_index as i32) * seq.step,
                seq.pad_width,
            );
            let sep = seq.separator.as_deref().unwrap_or("");
            name = apply_insert(&name, seq.insert_index.unwrap_or(0), &format!("{sep}{num}"));
        }
    }

    if let Some(ref prefix) = rules.prefix {
        name = format!("{prefix}{name}");
    }
    if let Some(ref suffix) = rules.suffix {
        name = format!("{name}{suffix}");
    }

    if let Some(ref seq) = rules.sequence {
        if seq.enabled {
            let num = format_sequence(
                seq.start + (seq_index as i32) * seq.step,
                seq.pad_width,
            );
            let sep = seq.separator.as_deref().unwrap_or("");
            match seq.position.as_str() {
                "prefix" => name = format!("{num}{sep}{name}"),
                "suffix" => name = format!("{name}{sep}{num}"),
                _ => {}
            }
        }
    }

    name = sanitize_name(&name);
    if name.is_empty() {
        return (String::new(), Some("处理后名称为空".into()));
    }
    (format!("{name}{ext}"), None)
}

fn should_ignore_rename(name: &str, ignore_patterns: &[String]) -> bool {
    if name == ".DS_Store" || name == ".git" {
        return true;
    }
    ignore_patterns.iter().any(|p| name.contains(p))
}

fn collect_entries(
    root: &Path,
    recursive: bool,
    scope: &str,
    ignore_patterns: &[String],
) -> Vec<RenameEntry> {
    let mut entries = Vec::new();

    fn walk(
        dir: &Path,
        rel_dir: &str,
        recursive: bool,
        scope: &str,
        ignore_patterns: &[String],
        entries: &mut Vec<RenameEntry>,
    ) {
        let mut list: Vec<_> = match fs::read_dir(dir) {
            Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
            Err(_) => return,
        };
        list.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

        for ent in list {
            let name = ent.file_name().to_string_lossy().into_owned();
            if should_ignore_rename(&name, ignore_patterns) {
                continue;
            }
            let rel = if rel_dir.is_empty() {
                name.clone()
            } else {
                format!("{rel_dir}/{name}")
            };
            let full = ent.path();
            let ft = match ent.file_type() {
                Ok(t) => t,
                Err(_) => continue,
            };

            if ft.is_dir() {
                if scope == "directories" || scope == "both" {
                    entries.push(RenameEntry {
                        relative_path: rel.clone(),
                        name: name.clone(),
                        kind: "directory".into(),
                        parent: rel_dir.to_string(),
                        absolute_path: full.to_string_lossy().into_owned(),
                    });
                }
                if recursive {
                    walk(&full, &rel, recursive, scope, ignore_patterns, entries);
                }
            } else if ft.is_file() && (scope == "files" || scope == "both") {
                entries.push(RenameEntry {
                    relative_path: rel,
                    name,
                    kind: "file".into(),
                    parent: rel_dir.to_string(),
                    absolute_path: full.to_string_lossy().into_owned(),
                });
            }
        }
    }

    walk(root, "", recursive, scope, ignore_patterns, &mut entries);
    entries
}

pub fn build_rename_plan(
    root: &Path,
    rules: &RenameRules,
    recursive: bool,
    scope: &str,
    ignore_patterns: &[String],
) -> Vec<RenamePlanItem> {
    let entries = collect_entries(root, recursive, scope, ignore_patterns);
    build_plan_from_entries(&entries, rules)
}

pub fn build_rename_plan_for_paths(
    root: &Path,
    relative_paths: &[String],
    rules: &RenameRules,
) -> Vec<RenamePlanItem> {
    let selected: std::collections::HashSet<String> = relative_paths.iter().cloned().collect();
    let entries: Vec<_> = collect_entries(root, true, "files", &[])
        .into_iter()
        .filter(|e| selected.contains(&e.relative_path))
        .collect();
    build_plan_from_entries(&entries, rules)
}

fn build_plan_from_entries(entries: &[RenameEntry], rules: &RenameRules) -> Vec<RenamePlanItem> {
    let mut plan = Vec::new();
    let mut used_names: HashMap<String, String> = HashMap::new();

    for (idx, entry) in entries.iter().enumerate() {
        let (new_name, error) = transform_file_name(&entry.name, rules, idx);
        let new_relative = if entry.parent.is_empty() {
            new_name.clone()
        } else {
            format!("{}/{}", entry.parent, new_name)
        };

        let (status, reason) = if let Some(err) = error {
            ("invalid".to_string(), Some(err))
        } else if new_name == entry.name {
            ("unchanged".to_string(), Some("名称未变化".into()))
        } else {
            let key = format!("{}\0{}", entry.parent, new_name);
            if let Some(collision_with) = used_names.get(&key) {
                (
                    "collision".to_string(),
                    Some(format!("与「{collision_with}」重名")),
                )
            } else {
                let exists = entries.iter().any(|e| {
                    e.parent == entry.parent && e.name == new_name && e.relative_path != entry.relative_path
                });
                if exists {
                    ("collision".to_string(), Some("目标名称已存在".into()))
                } else {
                    used_names.insert(key, entry.relative_path.clone());
                    ("ready".to_string(), None)
                }
            }
        };

        plan.push(RenamePlanItem {
            relative_path: entry.relative_path.clone(),
            old_name: entry.name.clone(),
            new_name,
            new_relative_path: new_relative,
            status,
            reason,
            kind: entry.kind.clone(),
        });
    }

    plan
}

pub fn execute_rename_plan(
    root: &Path,
    plan_items: Vec<RenamePlanItem>,
) -> (Vec<RenamePlanItem>, Vec<serde_json::Value>) {
    let mut renamed = Vec::new();
    let mut errors = Vec::new();

    let mut sorted: Vec<_> = plan_items
        .into_iter()
        .filter(|p| p.status == "ready")
        .collect();
    sorted.sort_by(|a, b| b.relative_path.len().cmp(&a.relative_path.len()));

    for item in sorted {
        let old_path = root.join(&item.relative_path);
        let new_path = root.join(&item.new_relative_path);
        if !old_path.exists() {
            let mut val = serde_json::to_value(&item).unwrap();
            if let Some(obj) = val.as_object_mut() {
                obj.insert("message".into(), serde_json::json!("源不存在"));
            }
            errors.push(val);
            continue;
        }
        if new_path.exists() {
            let mut val = serde_json::to_value(&item).unwrap();
            if let Some(obj) = val.as_object_mut() {
                obj.insert("message".into(), serde_json::json!("目标已存在"));
            }
            errors.push(val);
            continue;
        }
        match fs::rename(&old_path, &new_path) {
            Ok(()) => renamed.push(item),
            Err(e) => {
                let mut val = serde_json::to_value(&item).unwrap();
                if let Some(obj) = val.as_object_mut() {
                    obj.insert("message".into(), serde_json::json!(e.to_string()));
                }
                errors.push(val);
            }
        }
    }

    (renamed, errors)
}

pub fn rename_stats(plan: &[RenamePlanItem]) -> RenameStats {
    RenameStats {
        total: plan.len(),
        ready: plan.iter().filter(|p| p.status == "ready").count(),
        unchanged: plan.iter().filter(|p| p.status == "unchanged").count(),
        collision: plan.iter().filter(|p| p.status == "collision").count(),
        invalid: plan.iter().filter(|p| p.status == "invalid").count(),
    }
}

pub fn sanitize_names_plan(root: &Path, scope: &str, ignore_patterns: &[String]) -> Vec<RenamePlanItem> {
    let rules = RenameRules {
        replacements: Some(vec![
            crate::models::RenameReplacement { from: ":".into(), to: Some("_".into()) },
            crate::models::RenameReplacement { from: "：".into(), to: Some("_".into()) },
            crate::models::RenameReplacement { from: "/".into(), to: Some("_".into()) },
            crate::models::RenameReplacement { from: "／".into(), to: Some("_".into()) },
            crate::models::RenameReplacement { from: "\\".into(), to: Some("_".into()) },
        ]),
        ..Default::default()
    };
    build_rename_plan(root, &rules, true, scope, ignore_patterns)
}
