import { invoke } from '@tauri-apps/api/core';
import type {
  AppConfig,
  CompareMode,
  CompareResult,
  DuplicateGroup,
  FavoriteItem,
  FindFileEntry,
  FindFilesMatchMode,
  FolderItem,
  RenamePlanItem,
  RenameRules,
  SyncPreviewOperation,
  SyncPreviewSummary,
  SyncStrategy,
} from '@/types';
import {
  YuqueProgressState,
  loadAppConfig,
  loadYuqueProgress,
  saveAppConfigPartial,
  saveFavoriteAction,
  saveYuqueProgress,
} from '@/utils/appStorage';

function formatInvokeError(err: unknown): string {
  if (err instanceof Error && err.message.trim()) return err.message.trim();
  if (typeof err === 'string' && err.trim()) return err.trim();
  if (err && typeof err === 'object' && 'message' in err) {
    const message = (err as { message?: unknown }).message;
    if (typeof message === 'string' && message.trim()) return message.trim();
  }
  const fallback = String(err ?? '').trim();
  return fallback || '操作失败，请稍后重试';
}

async function invokeOk<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (err) {
    throw new Error(formatInvokeError(err));
  }
}

export async function pickFolder() {
  const data = await invokeOk<{ cancelled?: boolean; path?: string; name?: string }>('pick_folder');
  if (data.cancelled) return { cancelled: true as const, path: '', name: '' };
  return { path: data.path || '', name: data.name || '' };
}

export function getConfig() {
  return Promise.resolve({ config: loadAppConfig() });
}

export function saveConfig(partial: Partial<AppConfig>) {
  return Promise.resolve({ config: saveAppConfigPartial(partial) });
}

export function saveFavorite(action: 'add' | 'remove' | 'update', favorite: Partial<FavoriteItem> & { id?: string }) {
  return Promise.resolve({ favorites: saveFavoriteAction(action, favorite) });
}

export async function compareFolders(folders: FolderItem[], mode: CompareMode, ignorePatterns?: string[]) {
  return invokeOk<CompareResult>('compare_folders', { folders, mode, ignorePatterns });
}

export async function previewSyncFolders(params: {
  strategy: SyncStrategy;
  folders: FolderItem[];
  relativePaths: string[];
  deleteExtra?: boolean;
  sourceFolderId?: string;
  targetFolderId?: string;
}) {
  return invokeOk<{ operations: SyncPreviewOperation[]; summary: SyncPreviewSummary }>('preview_sync', { params });
}

export async function syncFolders(params: {
  strategy: SyncStrategy;
  folders: FolderItem[];
  relativePaths: string[];
  deleteExtra?: boolean;
  sourceFolderId?: string;
  targetFolderId?: string;
}) {
  return invokeOk<{ results: unknown[] }>('sync_folders', { params });
}

export async function deleteFiles(items: { folderPath: string; relativePath: string }[]) {
  return invokeOk<{ deleted: unknown[]; errors: unknown[] }>('delete_files', { items });
}

export async function moveFiles(items: {
  fromFolderPath: string;
  toFolderPath: string;
  relativePath: string;
  targetRelativePath?: string;
}[]) {
  return invokeOk<{ moved: unknown[]; errors: unknown[] }>('move_files', { items });
}

export async function openFolder(folderPath: string) {
  return invokeOk<{ ok: true }>('open_folder', { folderPath });
}

export async function previewRename(params: {
  rootPath: string;
  rules: RenameRules;
  recursive?: boolean;
  scope?: 'files' | 'directories' | 'both';
  ignorePatterns?: string[];
}) {
  const ignorePatterns = params.ignorePatterns ?? loadAppConfig().settings.ignorePatterns;
  return invokeOk<{ plan: RenamePlanItem[]; stats: Record<string, number> }>('preview_rename', {
    ...params,
    ignorePatterns,
  });
}

export async function executeRename(rootPath: string, items: RenamePlanItem[]) {
  return invokeOk<{ renamed: RenamePlanItem[]; errors: unknown[] }>('execute_rename', { rootPath, items });
}

export async function sanitizeNames(
  rootPath: string,
  scope: 'files' | 'directories' | 'both' = 'both',
  ignorePatterns?: string[],
) {
  const patterns = ignorePatterns ?? loadAppConfig().settings.ignorePatterns;
  return invokeOk<{ renamed: RenamePlanItem[]; errors: unknown[]; planned: number }>('sanitize_names', {
    rootPath,
    scope,
    ignorePatterns: patterns,
  });
}

export async function findDuplicates(rootPath: string, ignorePatterns?: string[]) {
  const patterns = ignorePatterns ?? loadAppConfig().settings.ignorePatterns;
  return invokeOk<{ groups: DuplicateGroup[]; stats: { groupCount: number; duplicateFiles: number; wastedBytes: number } }>(
    'find_duplicates',
    { rootPath, ignorePatterns: patterns },
  );
}

export async function findFiles(params: {
  rootPath: string;
  matchMode: FindFilesMatchMode;
  pattern: string;
  caseSensitive?: boolean;
  matchFullPath?: boolean;
  minSize?: number;
  maxSize?: number;
  ignorePatterns?: string[];
}) {
  const patterns = params.ignorePatterns ?? loadAppConfig().settings.ignorePatterns;
  return invokeOk<{
    files: FindFileEntry[];
    stats: { count: number; totalBytes: number };
  }>('find_files', {
    params: {
      rootPath: params.rootPath,
      matchMode: params.matchMode,
      pattern: params.pattern,
      caseSensitive: params.caseSensitive ?? false,
      matchFullPath: params.matchFullPath ?? false,
      minSize: params.minSize,
      maxSize: params.maxSize,
      ignorePatterns: patterns,
    },
  });
}

export async function previewRenameSelected(params: {
  rootPath: string;
  relativePaths: string[];
  rules: RenameRules;
}) {
  return invokeOk<{ plan: RenamePlanItem[]; stats: Record<string, number> }>('preview_rename_selected', params);
}

export function formatSize(bytes: number | null | undefined): string {
  if (bytes == null) return '—';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

export function uid() {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
}

export function defaultFolders(): FolderItem[] {
  return [
    { id: uid(), path: '', label: '文件夹 A', isPrimary: true },
    { id: uid(), path: '', label: '文件夹 B', isPrimary: false },
  ];
}

export async function getHealth() {
  return invokeOk<{ ok: boolean; version: number; features: string[] }>('get_health');
}

export async function previewYuque(url: string, standardMarkdown = true) {
  return invokeOk<{ title: string; preview: string; imageCount: number; charCount: number }>('preview_yuque', {
    url,
    standardMarkdown,
  });
}

export async function previewYuqueBook(url: string, token?: string) {
  return invokeOk<{
    authMode: 'token' | 'share';
    bookName: string;
    total: number;
    docs: { title: string; slug: string; dirPath: string }[];
  }>('preview_yuque_book', { url, token: token || undefined });
}

export type YuqueExportFormat = 'md' | 'html' | 'both';

export async function exportYuque(params: {
  url: string;
  saveDir: string;
  downloadImages?: boolean;
  standardMarkdown?: boolean;
  useDocFolder?: boolean;
  exportFormat?: YuqueExportFormat;
  exportConfluenceHtml?: boolean;
}) {
  return invokeOk<{
    title: string;
    fileName: string;
    filePath: string;
    exportFormat: YuqueExportFormat;
    mdPath?: string | null;
    mdFileName?: string | null;
    htmlPath?: string | null;
    htmlFileName?: string | null;
    folderPath: string | null;
    imageCount: number;
    downloadedImages: number;
    charCount: number;
  }>('export_yuque', { params });
}

export async function fetchYuqueExportProgress(url: string, saveDir: string, token?: string) {
  const progress = loadYuqueProgress(url, saveDir) || undefined;
  const data = await invokeOk<{
    found: boolean;
    bookName?: string;
    bookDir?: string;
    total?: number;
    completed?: number;
    remaining?: number;
    failedCount?: number;
    status?: string;
    updatedAt?: string;
    startedAt?: string;
    currentSlug?: string | null;
    completedSlugs?: string[];
    failed?: { slug: string; title?: string; message: string }[];
    docs?: {
      slug: string;
      title: string;
      dirPath: string;
      status: 'pending' | 'done' | 'failed' | 'exporting';
      failMessage?: string;
    }[];
    progress?: YuqueProgressState;
  }>('yuque_export_progress', { url, saveDir, token: token || undefined, progress });

  if (data.progress) {
    saveYuqueProgress(url, saveDir, data.progress);
  }
  return data;
}

export async function exportYuqueBatch(params: {
  url: string;
  saveDir: string;
  token?: string;
  resume?: boolean;
  downloadImages?: boolean;
  standardMarkdown?: boolean;
  exportFormat?: YuqueExportFormat;
  exportConfluenceHtml?: boolean;
  delayMode?: 'none' | 'fixed' | 'random';
  delayFixedSec?: number;
  delayMinSec?: number;
  delayMaxSec?: number;
  useDocFolder?: boolean;
  stopOnError?: boolean;
  exportOrder?: 'top-down' | 'bottom-up' | 'custom';
  selectedSlugs?: string[];
}) {
  const storedProgress = params.resume !== false ? loadYuqueProgress(params.url, params.saveDir) : null;
  const result = await invokeOk<{
    bookName: string;
    bookDir: string;
    total: number;
    exported: number;
    newlyExported: number;
    skippedCount: number;
    remainingCount: number;
    failedCount: number;
    resume: boolean;
    stoppedEarly?: boolean;
    paused?: boolean;
    delayMode: string;
    success: { title: string; filePath: string; folderPath: string | null; relativePath?: string }[];
    failed: { title: string; slug: string; dirPath?: string; message: string }[];
    progress?: YuqueProgressState;
  }>('export_yuque_batch', { params: { ...params, progress: storedProgress || undefined } });

  if (result.progress) {
    saveYuqueProgress(params.url, params.saveDir, result.progress);
  }
  return result;
}

export function clearYuqueProgress(url: string, saveDir: string) {
  saveYuqueProgress(url, saveDir, null);
  return Promise.resolve({ clearedCount: 1 });
}

export async function cancelYuqueExport(url: string, saveDir: string) {
  return invokeOk<{ requested: boolean }>('cancel_yuque_export', { url, saveDir });
}

export async function resetYuqueExport(url: string, saveDir: string) {
  await invokeOk<{ cleared: boolean }>('reset_yuque_export', { url, saveDir });
  clearYuqueProgress(url, saveDir);
}

export async function importYuqueProgress(saveDir: string, progress: YuqueProgressState) {
  return invokeOk<{ imported: boolean }>('import_yuque_progress', { saveDir, progress });
}

export async function pickSaveFile(defaultName?: string) {
  const data = await invokeOk<{ cancelled?: boolean; path?: string }>('pick_save_file', {
    defaultName: defaultName || undefined,
  });
  if (data.cancelled) return { cancelled: true as const, path: '' };
  return { path: data.path || '' };
}

export async function pickOpenFile() {
  const data = await invokeOk<{ cancelled?: boolean; path?: string }>('pick_open_file');
  if (data.cancelled) return { cancelled: true as const, path: '' };
  return { path: data.path || '' };
}

export async function writeTextFile(path: string, content: string) {
  return invokeOk<{ path: string }>('write_text_file', { path, content });
}

export async function readTextFile(path: string) {
  return invokeOk<{ content: string }>('read_text_file', { path });
}

export async function listConfluenceFiles(sourceDir: string, recursive = true) {
  return invokeOk<{
    files: { absolutePath: string; relativePath: string; fileName: string }[];
    count: number;
  }>('confluence_list', { sourceDir, recursive });
}

export async function previewConfluenceFile(filePath: string) {
  return invokeOk<{
    filePath: string;
    fileName: string;
    title: string;
    charCount: number;
    html: string;
    bodyHtml: string;
    imagesEmbedded?: number;
    imagesFailed?: { url: string; message: string }[];
  }>('confluence_preview', { filePath });
}

export type ConfluenceOutputFormat = 'html' | 'docx' | 'md' | 'pdf';

export async function convertToConfluence(params: {
  sourceDir: string;
  outputDir?: string;
  sameDir?: boolean;
  recursive?: boolean;
  overwrite?: boolean;
  format?: ConfluenceOutputFormat;
  files?: string[];
}) {
  return invokeOk<{
    sourceDir: string;
    outputDir: string;
    outputFormat: ConfluenceOutputFormat;
    total: number;
    selectedCount?: number;
    allCount?: number;
    convertedCount: number;
    skippedCount: number;
    failedCount: number;
    converted: { relativePath: string; outputPath: string; title: string }[];
    skipped: { relativePath: string; outputPath: string }[];
    failed: { relativePath: string; message: string }[];
  }>('confluence_convert', { params });
}
