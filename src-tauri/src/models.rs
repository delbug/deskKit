use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompareMode {
    Name,
    #[serde(rename = "md5")]
    Md5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CompareExtensionMode {
    #[default]
    None,
    Include,
    Exclude,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderItem {
    pub id: String,
    pub path: String,
    pub label: String,
    pub is_primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderMeta {
    pub id: String,
    pub path: String,
    pub label: String,
    pub is_primary: bool,
    pub file_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMeta {
    pub relative_path: String,
    pub absolute_path: String,
    pub size: u64,
    pub mtime: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub md5: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffEntry {
    pub relative_path: String,
    pub status: String,
    pub presence: HashMap<String, bool>,
    pub sizes: HashMap<String, Option<u64>>,
    pub md5s: HashMap<String, Option<String>>,
    pub mtimes: HashMap<String, Option<f64>>,
    pub present_count: usize,
    pub primary_has: bool,
    pub primary_only: bool,
    pub secondary_only: bool,
    pub present_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paths_by_folder: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub md5: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareStats {
    pub total: usize,
    pub identical: usize,
    pub missing: usize,
    pub content_diff: usize,
    pub relocated: usize,
    pub only_in: HashMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareResult {
    pub mode: CompareMode,
    pub primary_id: String,
    pub folders: HashMap<String, FolderMeta>,
    pub entries: Vec<DiffEntry>,
    pub stats: CompareStats,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SyncStrategy {
    #[serde(rename = "primary-overwrite")]
    PrimaryOverwrite,
    Union,
    Selected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPreviewOperation {
    pub action: String,
    pub relative_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPreviewSummary {
    pub copy: usize,
    pub overwrite: usize,
    #[serde(rename = "delete")]
    pub delete: usize,
    pub skip: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncParams {
    pub strategy: SyncStrategy,
    pub folders: Vec<FolderItem>,
    #[serde(default)]
    pub relative_paths: Vec<String>,
    #[serde(default = "default_true")]
    pub delete_extra: bool,
    #[serde(default)]
    pub source_folder_id: Option<String>,
    #[serde(default)]
    pub target_folder_id: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteFileItem {
    pub folder_path: String,
    pub relative_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveFileItem {
    pub from_folder_path: String,
    pub to_folder_path: String,
    pub relative_path: String,
    #[serde(default)]
    pub target_relative_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameSequenceRules {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_position")]
    pub position: String,
    #[serde(default)]
    pub insert_index: Option<i32>,
    #[serde(default = "default_one")]
    pub start: i32,
    #[serde(default = "default_one")]
    pub step: i32,
    #[serde(default)]
    pub pad_width: i32,
    #[serde(default)]
    pub separator: Option<String>,
}

fn default_position() -> String {
    "suffix".to_string()
}

fn default_one() -> i32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameInsertRules {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub index: i32,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub use_sequence: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameDeleteAtRules {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_one")]
    pub start: i32,
    #[serde(default)]
    pub count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameReplacement {
    pub from: String,
    #[serde(default)]
    pub to: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameRules {
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default)]
    pub suffix: Option<String>,
    #[serde(default)]
    pub replace_from: Option<String>,
    #[serde(default)]
    pub replace_to: Option<String>,
    #[serde(default)]
    pub replacements: Option<Vec<RenameReplacement>>,
    #[serde(default)]
    pub remove_patterns: Option<Vec<String>>,
    #[serde(default)]
    pub include_extension: bool,
    #[serde(default)]
    pub sequence: Option<RenameSequenceRules>,
    #[serde(default)]
    pub insert: Option<RenameInsertRules>,
    #[serde(default)]
    pub delete_at: Option<RenameDeleteAtRules>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenamePlanItem {
    pub relative_path: String,
    pub old_name: String,
    pub new_name: String,
    pub new_relative_path: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameStats {
    pub total: usize,
    pub ready: usize,
    pub unchanged: usize,
    pub collision: usize,
    pub invalid: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateFileEntry {
    pub relative_path: String,
    pub absolute_path: String,
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub md5: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    pub md5: String,
    pub size: u64,
    pub count: usize,
    pub files: Vec<DuplicateFileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateStats {
    pub group_count: usize,
    pub duplicate_files: usize,
    pub wasted_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Md5ScanStats {
    pub total: usize,
    pub errors: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Md5ScanResult {
    pub root_path: String,
    pub is_file: bool,
    pub entries: Vec<FileMeta>,
    pub stats: Md5ScanStats,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Md5RenameMode {
    Prefix,
    Suffix,
    HashOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Md5RenameItem {
    pub relative_path: String,
    pub md5: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Md5RenameResult {
    pub renamed: usize,
    pub skipped: usize,
    pub dry_run: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Md5VerifyResult {
    pub matched: usize,
    pub mismatched: usize,
    pub missing: usize,
    pub total: usize,
    pub details: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindFilesParams {
    pub root_path: String,
    #[serde(default)]
    pub ignore_patterns: Vec<String>,
    /// name | suffix | extension | regex
    pub match_mode: String,
    pub pattern: String,
    #[serde(default)]
    pub case_sensitive: bool,
    #[serde(default)]
    pub match_full_path: bool,
    #[serde(default)]
    pub min_size: Option<u64>,
    #[serde(default)]
    pub max_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindFileEntry {
    pub relative_path: String,
    pub absolute_path: String,
    pub size: u64,
    pub mtime: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindFilesStats {
    pub count: usize,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum YuqueExportFormat {
    Md,
    Html,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YuqueExportParams {
    pub url: String,
    pub save_dir: String,
    #[serde(default = "default_true")]
    pub download_images: bool,
    #[serde(default = "default_true")]
    pub standard_markdown: bool,
    #[serde(default)]
    pub use_doc_folder: bool,
    #[serde(default)]
    pub export_format: Option<YuqueExportFormat>,
    #[serde(default)]
    pub export_confluence_html: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YuqueBatchParams {
    pub url: String,
    pub save_dir: String,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default = "default_true")]
    pub resume: bool,
    #[serde(default = "default_true")]
    pub download_images: bool,
    #[serde(default = "default_true")]
    pub standard_markdown: bool,
    #[serde(default)]
    pub use_doc_folder: bool,
    #[serde(default = "default_true")]
    pub stop_on_error: bool,
    #[serde(default)]
    pub export_format: Option<YuqueExportFormat>,
    #[serde(default)]
    pub export_confluence_html: bool,
    #[serde(default = "default_delay_mode")]
    pub delay_mode: String,
    #[serde(default = "default_five")]
    pub delay_fixed_sec: u64,
    #[serde(default = "default_three")]
    pub delay_min_sec: u64,
    #[serde(default = "default_thirty")]
    pub delay_max_sec: u64,
    #[serde(default)]
    pub progress: Option<YuqueProgressState>,
    #[serde(default = "default_export_order")]
    pub export_order: String,
    #[serde(default)]
    pub selected_slugs: Option<Vec<String>>,
    /// 每轮最多新导出篇数，达到后自动暂停；None 或 0 表示不限制
    #[serde(default)]
    pub batch_limit: Option<u32>,
}

fn default_export_order() -> String {
    "top-down".to_string()
}

fn default_delay_mode() -> String {
    "random".to_string()
}

fn default_five() -> u64 {
    5
}

fn default_three() -> u64 {
    3
}

fn default_thirty() -> u64 {
    30
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YuqueDocManifestItem {
    pub slug: String,
    pub title: String,
    pub dir_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YuqueFailedItem {
    pub slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YuqueProgressState {
    #[serde(default)]
    pub version: i32,
    pub url: String,
    #[serde(default)]
    pub auth_mode: Option<String>,
    #[serde(default)]
    pub namespace: Option<String>,
    #[serde(default)]
    pub book_name: Option<String>,
    #[serde(default)]
    pub book_dir: Option<String>,
    #[serde(default)]
    pub save_dir: Option<String>,
    #[serde(default)]
    pub total: Option<usize>,
    #[serde(default)]
    pub completed_slugs: Option<Vec<String>>,
    #[serde(default)]
    pub failed: Option<Vec<YuqueFailedItem>>,
    #[serde(default)]
    pub doc_manifest: Option<Vec<YuqueDocManifestItem>>,
    #[serde(default)]
    pub current_slug: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub export_order: Option<String>,
    #[serde(default)]
    pub selected_slugs: Option<Vec<String>>,
    #[serde(default)]
    pub duplicate_slugs: Option<Vec<String>>,
    /// 篇间等待结束时间（RFC3339），供前端显示倒计时
    #[serde(default)]
    pub delay_until: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YuqueDocPreview {
    pub title: String,
    pub slug: String,
    pub dir_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_type_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfluenceFileEntry {
    pub absolute_path: String,
    pub relative_path: String,
    pub file_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfluenceOutputFormat {
    Html,
    Docx,
    Md,
    Pdf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfluenceConvertParams {
    pub source_dir: String,
    #[serde(default)]
    pub output_dir: Option<String>,
    #[serde(default)]
    pub same_dir: bool,
    #[serde(default = "default_true")]
    pub recursive: bool,
    #[serde(default)]
    pub overwrite: bool,
    #[serde(default)]
    pub format: Option<ConfluenceOutputFormat>,
    #[serde(default)]
    pub files: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub ok: bool,
    pub version: i32,
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PickFileResponse {
    #[serde(default)]
    pub cancelled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PickFolderResponse {
    #[serde(default)]
    pub cancelled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
