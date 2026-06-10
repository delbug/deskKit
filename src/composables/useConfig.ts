import { ref } from 'vue';
import type { AppConfig } from '@/types';
import { uid } from '@/api';
import {
  DEFAULT_APP_CONFIG,
  STORAGE_LABEL,
  loadAppConfig,
  saveAppConfigPartial,
  saveFavoriteAction,
} from '@/utils/appStorage';

const config = ref<AppConfig | null>(null);
const loaded = ref(false);

export function useConfig() {
  async function load() {
    config.value = loadAppConfig();
    loaded.value = true;
    return config.value;
  }

  async function persist(partial: Partial<AppConfig>) {
    config.value = saveAppConfigPartial(partial);
    return config.value;
  }

  async function saveLastSession(
    folders: AppConfig['lastSession']['folders'],
    compareMode: AppConfig['lastSession']['compareMode'],
  ) {
    return persist({ lastSession: { folders, compareMode } });
  }

  async function saveFavorite(
    action: 'add' | 'remove' | 'update',
    favorite: Parameters<typeof saveFavoriteAction>[1],
  ) {
    const favorites = saveFavoriteAction(action, favorite);
    config.value = loadAppConfig();
    return favorites;
  }

  function restoreFolders() {
    const session = config.value?.lastSession;
    const folders = session?.folders;
    if (folders && folders.length >= 2) {
      return folders.map((f) => ({ ...f, id: f.id || uid() }));
    }
    return null;
  }

  return {
    config,
    storageLabel: STORAGE_LABEL,
    defaultConfig: DEFAULT_APP_CONFIG,
    loaded,
    load,
    persist,
    saveLastSession,
    saveFavorite,
    restoreFolders,
  };
}
