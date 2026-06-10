use crate::fs_util::walk_files;
use crate::models::{FindFileEntry, FindFilesParams, FindFilesStats};
use regex::Regex;
use std::path::Path;

fn file_name_from_rel(relative_path: &str) -> &str {
    Path::new(relative_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(relative_path)
}

fn glob_to_regex(glob: &str) -> String {
    let mut re = String::from("^");
    for c in glob.chars() {
        match c {
            '*' => re.push_str(".*"),
            '?' => re.push('.'),
            '.' | '+' | '(' | ')' | '|' | '^' | '$' | '[' | ']' | '{' | '}' | '\\' => {
                re.push('\\');
                re.push(c);
            }
            _ => re.push(c),
        }
    }
    re.push('$');
    re
}

fn parse_extensions(pattern: &str) -> Vec<String> {
    pattern
        .split([',', '，', ';', '；', ' ', '\n', '\t'])
        .map(|s| s.trim().trim_start_matches('.').to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

fn extension_of(name: &str) -> Option<String> {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
}

fn compile_regex(pattern: &str, case_sensitive: bool) -> Result<Regex, String> {
    let flags = if case_sensitive { "" } else { "(?i)" };
    Regex::new(&format!("{flags}{pattern}")).map_err(|e| format!("正则无效: {e}"))
}

fn matches_name(pattern: &str, file_name: &str, case_sensitive: bool) -> Result<bool, String> {
    if pattern.is_empty() {
        return Ok(true);
    }
    if pattern.contains('*') || pattern.contains('?') {
        let re_pat = glob_to_regex(pattern);
        let re = compile_regex(&re_pat, case_sensitive)?;
        return Ok(re.is_match(file_name));
    }
    if case_sensitive {
        Ok(file_name.contains(pattern))
    } else {
        Ok(file_name.to_lowercase().contains(&pattern.to_lowercase()))
    }
}

fn matches_suffix(pattern: &str, file_name: &str, case_sensitive: bool) -> bool {
    if pattern.is_empty() {
        return true;
    }
    if case_sensitive {
        file_name.ends_with(pattern)
    } else {
        file_name.to_lowercase().ends_with(&pattern.to_lowercase())
    }
}

fn matches_extension(pattern: &str, file_name: &str) -> bool {
    let exts = parse_extensions(pattern);
    if exts.is_empty() {
        return true;
    }
    let Some(ext) = extension_of(file_name) else {
        return false;
    };
    exts.iter().any(|e| e == &ext)
}

fn matches_regex(
    re: &Regex,
    file_name: &str,
    relative_path: &str,
    match_full_path: bool,
) -> bool {
    if match_full_path {
        re.is_match(relative_path)
    } else {
        re.is_match(file_name)
    }
}

pub fn find_files(root: &Path, params: &FindFilesParams) -> Result<(Vec<FindFileEntry>, FindFilesStats), String> {
    if params.pattern.trim().is_empty() && params.match_mode != "extension" {
        return Err("请填写匹配内容".into());
    }

    let regex = if params.match_mode == "regex" {
        Some(compile_regex(params.pattern.trim(), params.case_sensitive)?)
    } else {
        None
    };

    let files = walk_files(root, &params.ignore_patterns);
    let mut matched = Vec::new();

    for (relative_path, meta) in files {
        if let Some(min) = params.min_size {
            if meta.size < min {
                continue;
            }
        }
        if let Some(max) = params.max_size {
            if meta.size > max {
                continue;
            }
        }

        let file_name = file_name_from_rel(&relative_path);
        let ok = match params.match_mode.as_str() {
            "name" => matches_name(params.pattern.trim(), file_name, params.case_sensitive)?,
            "suffix" => matches_suffix(params.pattern.trim(), file_name, params.case_sensitive),
            "extension" => matches_extension(params.pattern.trim(), file_name),
            "regex" => matches_regex(
                regex.as_ref().unwrap(),
                file_name,
                &relative_path,
                params.match_full_path,
            ),
            other => return Err(format!("不支持的匹配模式: {other}")),
        };

        if ok {
            matched.push(FindFileEntry {
                relative_path: meta.relative_path,
                absolute_path: meta.absolute_path,
                size: meta.size,
                mtime: meta.mtime,
            });
        }
    }

    matched.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    let total_bytes: u64 = matched.iter().map(|f| f.size).sum();
    let stats = FindFilesStats {
        count: matched.len(),
        total_bytes,
    };

    Ok((matched, stats))
}
