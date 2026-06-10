mod compare_sync;
mod confluence;
mod duplicates;
mod fs_util;
mod models;
mod rename_ops;
mod yuque;

use compare_sync::{
    compare_folders as run_compare_folders,
    delete_files as run_delete_files,
    move_files as run_move_files,
    preview_sync as run_preview_sync,
    sync_folders as run_sync_folders,
};
use duplicates::find_duplicates as run_find_duplicates;
use confluence::{batch_convert_markdown, preview_markdown_file};
use fs_util::resolve_safe_dir;
use models::*;
use rename_ops::{build_rename_plan, execute_rename_plan, rename_stats, sanitize_names_plan};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
fn get_health() -> HealthResponse {
    HealthResponse {
        ok: true,
        version: 1,
        features: vec![
            "compare".into(),
            "sync".into(),
            "sync-preview".into(),
            "delete".into(),
            "move".into(),
            "pick-folder".into(),
            "rename".into(),
            "favorites".into(),
            "duplicates".into(),
            "yuque".into(),
            "confluence".into(),
            "config".into(),
        ],
    }
}

#[tauri::command]
async fn pick_folder(app: tauri::AppHandle) -> Result<PickFolderResponse, String> {
    let path = app
        .dialog()
        .file()
        .set_title("选择文件夹")
        .blocking_pick_folder();

    match path {
        Some(p) => {
            let path_str = p.to_string();
            let name = std::path::Path::new(&path_str)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            Ok(PickFolderResponse {
                cancelled: false,
                path: Some(path_str),
                name: Some(name),
            })
        }
        None => Ok(PickFolderResponse {
            cancelled: true,
            path: None,
            name: None,
        }),
    }
}

#[tauri::command]
async fn open_folder(app: tauri::AppHandle, folder_path: String) -> Result<serde_json::Value, String> {
    let resolved = resolve_safe_dir(&folder_path).map_err(|e| e.to_string())?;
    app.opener()
        .open_path(resolved.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "ok": true }))
}

#[tauri::command]
async fn compare_folders(
    folders: Vec<FolderItem>,
    mode: CompareMode,
    ignore_patterns: Option<Vec<String>>,
) -> Result<CompareResult, String> {
    run_compare_folders(folders, mode, ignore_patterns.unwrap_or_default())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn preview_sync(params: SyncParams) -> Result<serde_json::Value, String> {
    let (operations, summary) = run_preview_sync(params).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "operations": operations, "summary": summary }))
}

#[tauri::command]
fn sync_folders(params: SyncParams) -> Result<serde_json::Value, String> {
    let results = run_sync_folders(params).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "results": results }))
}

#[tauri::command]
fn delete_files(items: Vec<DeleteFileItem>) -> Result<serde_json::Value, String> {
    let (deleted, errors) = run_delete_files(items).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "deleted": deleted, "errors": errors }))
}

#[tauri::command]
fn move_files(items: Vec<MoveFileItem>) -> Result<serde_json::Value, String> {
    let (moved, errors) = run_move_files(items).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "moved": moved, "errors": errors }))
}

#[tauri::command]
fn preview_rename(
    root_path: String,
    rules: RenameRules,
    recursive: Option<bool>,
    scope: Option<String>,
    ignore_patterns: Option<Vec<String>>,
) -> Result<serde_json::Value, String> {
    let root = resolve_safe_dir(&root_path).map_err(|e| e.to_string())?;
    let plan = build_rename_plan(
        &root,
        &rules,
        recursive.unwrap_or(true),
        scope.as_deref().unwrap_or("files"),
        &ignore_patterns.unwrap_or_default(),
    );
    let stats = rename_stats(&plan);
    Ok(serde_json::json!({ "plan": plan, "stats": stats }))
}

#[tauri::command]
fn execute_rename(root_path: String, items: Vec<RenamePlanItem>) -> Result<serde_json::Value, String> {
    let root = resolve_safe_dir(&root_path).map_err(|e| e.to_string())?;
    let ready: Vec<_> = items.into_iter().filter(|p| p.status == "ready").collect();
    if ready.is_empty() {
        return Err("没有可执行的重命名项".into());
    }
    let (renamed, errors) = execute_rename_plan(&root, ready);
    Ok(serde_json::json!({ "renamed": renamed, "errors": errors }))
}

#[tauri::command]
fn sanitize_names(
    root_path: String,
    scope: Option<String>,
    ignore_patterns: Option<Vec<String>>,
) -> Result<serde_json::Value, String> {
    let root = resolve_safe_dir(&root_path).map_err(|e| e.to_string())?;
    let scope = scope.as_deref().unwrap_or("both");
    let patterns = ignore_patterns.unwrap_or_default();
    let plan = sanitize_names_plan(&root, scope, &patterns);
    let ready: Vec<_> = plan.iter().filter(|p| p.status == "ready").cloned().collect();
    let planned = ready.len();
    let (renamed, errors) = execute_rename_plan(&root, ready);
    Ok(serde_json::json!({ "renamed": renamed, "errors": errors, "planned": planned }))
}

#[tauri::command]
fn find_duplicates(
    root_path: String,
    ignore_patterns: Option<Vec<String>>,
) -> Result<serde_json::Value, String> {
    let root = resolve_safe_dir(&root_path).map_err(|e| e.to_string())?;
    let (groups, stats) = run_find_duplicates(&root, &ignore_patterns.unwrap_or_default())?;
    Ok(serde_json::json!({ "groups": groups, "stats": stats }))
}

#[tauri::command]
async fn preview_yuque(url: String, standard_markdown: Option<bool>) -> Result<serde_json::Value, String> {
    yuque::preview_yuque(&url, standard_markdown.unwrap_or(true)).await
}

#[tauri::command]
async fn preview_yuque_book(url: String, token: Option<String>) -> Result<serde_json::Value, String> {
    yuque::preview_yuque_book(&url, token).await
}

#[tauri::command]
async fn export_yuque(params: YuqueExportParams) -> Result<serde_json::Value, String> {
    yuque::export_yuque_doc(params).await
}

#[tauri::command]
async fn export_yuque_batch(params: YuqueBatchParams) -> Result<serde_json::Value, String> {
    yuque::export_yuque_batch(params).await
}

#[tauri::command]
fn yuque_export_progress(
    url: String,
    save_dir: String,
    token: Option<String>,
    progress: Option<YuqueProgressState>,
) -> Result<serde_json::Value, String> {
    let auth_mode = token
        .as_ref()
        .map(|t| if t.trim().is_empty() { "share" } else { "token" })
        .or(Some("share"));
    Ok(yuque::get_export_progress_summary(
        &save_dir,
        &url,
        auth_mode,
        progress,
    ))
}

#[tauri::command]
fn cancel_yuque_export(url: String, save_dir: String) -> Result<serde_json::Value, String> {
    yuque::cancel_yuque_export(&save_dir, &url);
    Ok(serde_json::json!({ "requested": true }))
}

#[tauri::command]
fn reset_yuque_export(url: String, save_dir: String) -> Result<serde_json::Value, String> {
    yuque::reset_yuque_export(&save_dir, &url);
    Ok(serde_json::json!({ "cleared": true }))
}

#[tauri::command]
fn confluence_list(source_dir: String, recursive: Option<bool>) -> Result<serde_json::Value, String> {
    confluence::confluence_list(&source_dir, recursive.unwrap_or(true))
}

#[tauri::command]
async fn confluence_preview(file_path: String) -> Result<serde_json::Value, String> {
    preview_markdown_file(&file_path).await
}

#[tauri::command]
async fn confluence_convert(params: ConfluenceConvertParams) -> Result<serde_json::Value, String> {
    batch_convert_markdown(params).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            get_health,
            pick_folder,
            open_folder,
            compare_folders,
            preview_sync,
            sync_folders,
            delete_files,
            move_files,
            preview_rename,
            execute_rename,
            sanitize_names,
            find_duplicates,
            preview_yuque,
            preview_yuque_book,
            export_yuque,
            export_yuque_batch,
            yuque_export_progress,
            cancel_yuque_export,
            reset_yuque_export,
            confluence_list,
            confluence_preview,
            confluence_convert,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
