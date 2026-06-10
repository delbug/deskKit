import { ref } from 'vue';
import { ElMessageBox } from 'element-plus';
import {
  clearYuqueProgressStorage,
  loadAppConfig,
  saveAppConfigPartial,
} from '@/utils/appStorage';

export type ModuleId =
  | 'compare'
  | 'rename'
  | 'favorites'
  | 'duplicates'
  | 'findFiles'
  | 'yuque'
  | 'confluence'
  | 'settings';

export const MODULE_STORAGE_KEYS: Record<ModuleId, string[]> = {
  compare: [],
  rename: [],
  favorites: [],
  duplicates: ['dup-last-path'],
  findFiles: ['find-files-last-path', 'find-files-match-mode', 'find-files-pattern', 'find-files-size-enabled', 'find-files-size-unit', 'find-files-min-size', 'find-files-max-size'],
  yuque: [
    'yuque-last-url',
    'yuque-save-dir',
    'yuque-export-mode',
    'yuque-auth-mode',
    'yuque-token',
    'yuque-image-mode',
    'yuque-standard-markdown',
    'yuque-use-doc-folder',
    'yuque-delay-mode',
    'yuque-delay-fixed',
    'yuque-delay-min',
    'yuque-delay-max',
    'yuque-resume-export',
    'yuque-stop-on-error',
    'yuque-export-md',
    'yuque-export-html',
    'yuque-export-format',
    'yuque-export-confluence-html',
    'deskit-yuque-progress',
    'sync-file-yuque-progress',
  ],
  confluence: [
    'confluence-source-dir',
    'confluence-output-dir',
    'confluence-same-dir',
    'confluence-recursive',
    'confluence-overwrite',
    'confluence-output-format',
    'confluence-selected-files',
    'confluence-preview-file',
  ],
  settings: [],
};

export const MODULE_LABELS: Record<ModuleId, string> = {
  compare: '文件夹对比',
  rename: '批量重命名',
  favorites: '收藏管理',
  duplicates: '重复文件',
  findFiles: '查找文件',
  yuque: '语雀导出',
  confluence: 'Confluence 转换',
  settings: '设置',
};

export function clearLocalStorageKeys(keys: string[]) {
  for (const key of keys) {
    localStorage.removeItem(key);
  }
}

export function clearModuleBrowserData(
  module: ModuleId,
  extra?: { url?: string; saveDir?: string },
) {
  if (module === 'compare') {
    saveAppConfigPartial({
      lastSession: { folders: [], compareMode: 'md5' },
    });
  } else if (module === 'favorites') {
    saveAppConfigPartial({ favorites: [], pathFavorites: [], urlFavorites: [] });
  } else if (module === 'yuque') {
    if (extra?.url && extra?.saveDir) {
      clearYuqueProgressStorage(extra.url, extra.saveDir);
    } else {
      clearYuqueProgressStorage();
    }
  } else if (module === 'settings') {
    // 设置页无独立缓存，保留已保存配置
  }
}

export function useModuleClear(module: ModuleId) {
  const clearing = ref(false);

  async function confirmAndClear(onReset?: () => void | Promise<void>, extra?: { url?: string; saveDir?: string }) {
    try {
      await ElMessageBox.confirm(
        `将清除「${MODULE_LABELS[module]}」的所有历史记录与本地缓存，此操作不可恢复。`,
        '清除历史数据',
        { type: 'warning', confirmButtonText: '确认清除', cancelButtonText: '取消' },
      );
    } catch {
      return false;
    }

    clearing.value = true;
    try {
      clearLocalStorageKeys(MODULE_STORAGE_KEYS[module]);
      clearModuleBrowserData(module, extra);
      if (onReset) await onReset();
      return true;
    } finally {
      clearing.value = false;
    }
  }

  return { clearing, confirmAndClear };
}

export { loadAppConfig };
