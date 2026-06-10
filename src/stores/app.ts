import { defineStore } from 'pinia';
import type { AppConfig, CompareMode, FolderItem } from '@/types';
import {
  DEFAULT_APP_CONFIG,
  loadAppConfig,
  saveAppConfigPartial,
  saveFavoriteAction,
  type YuqueProgressState,
  loadYuqueProgress,
  saveYuqueProgress,
  clearYuqueProgressStorage,
} from '@/utils/appStorage';
import type { FavoriteItem } from '@/types';

export const useAppStore = defineStore('app', {
  state: (): { config: AppConfig } => ({
    config: loadAppConfig(),
  }),
  actions: {
    reloadConfig() {
      this.config = loadAppConfig();
    },
    savePartial(partial: Partial<AppConfig>) {
      this.config = saveAppConfigPartial(partial);
    },
    saveLastSession(folders: FolderItem[], compareMode: CompareMode) {
      this.savePartial({ lastSession: { folders, compareMode } });
    },
    restoreFolders(): FolderItem[] {
      const folders = this.config.lastSession?.folders || [];
      return folders.length >= 2 ? folders : [];
    },
    saveFavorite(action: 'add' | 'remove' | 'update', favorite: Partial<FavoriteItem> & { id?: string }) {
      const favorites = saveFavoriteAction(action, favorite);
      this.config = { ...this.config, favorites };
      return favorites;
    },
    resetConfig() {
      this.config = structuredClone(DEFAULT_APP_CONFIG);
      saveAppConfigPartial(this.config);
    },
    loadYuqueProgress(url: string, saveDir: string) {
      return loadYuqueProgress(url, saveDir);
    },
    saveYuqueProgress(url: string, saveDir: string, progress: YuqueProgressState | null) {
      saveYuqueProgress(url, saveDir, progress);
    },
    clearYuqueProgress(url?: string, saveDir?: string) {
      clearYuqueProgressStorage(url, saveDir);
    },
  },
});
