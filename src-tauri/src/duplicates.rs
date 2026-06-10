use crate::fs_util::{md5_file, walk_files};
use crate::models::{DuplicateGroup, DuplicateStats};
use std::path::Path;

pub fn find_duplicates(
    root: &Path,
    ignore_patterns: &[String],
) -> Result<(Vec<DuplicateGroup>, DuplicateStats), String> {
    let files = walk_files(root, ignore_patterns);
    let mut by_hash: std::collections::HashMap<String, Vec<(String, String, u64)>> =
        std::collections::HashMap::new();

    for (rel, meta) in &files {
        match md5_file(Path::new(&meta.absolute_path)) {
            Ok(hash) => {
                by_hash
                    .entry(hash)
                    .or_default()
                    .push((rel.clone(), meta.absolute_path.clone(), meta.size));
            }
            Err(_) => {}
        }
    }

    let mut groups = Vec::new();
    for (hash, items) in by_hash {
        if items.len() > 1 {
            let size = items[0].2;
            groups.push(DuplicateGroup {
                md5: hash,
                size,
                count: items.len(),
                files: items
                    .into_iter()
                    .map(|(relative_path, absolute_path, size)| {
                        crate::models::DuplicateFileEntry {
                            relative_path,
                            absolute_path,
                            size,
                            md5: None,
                        }
                    })
                    .collect(),
            });
        }
    }

    groups.sort_by(|a, b| b.count.cmp(&a.count));

    let stats = DuplicateStats {
        group_count: groups.len(),
        duplicate_files: groups.iter().map(|g| g.count).sum(),
        wasted_bytes: groups.iter().map(|g| g.size * (g.count as u64 - 1)).sum(),
    };

    Ok((groups, stats))
}
