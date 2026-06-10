import type { AppConfig, FavoriteItem } from '@/types';

const CONFIG_KEY = 'deskit-app-config';
const YUQUE_PROGRESS_KEY = 'deskit-yuque-progress';
const LEGACY_CONFIG_KEY = 'sync-file-app-config';
const LEGACY_YUQUE_PROGRESS_KEY = 'sync-file-yuque-progress';

function migrateLegacyStorageKeys() {
  try {
    if (!localStorage.getItem(CONFIG_KEY) && localStorage.getItem(LEGACY_CONFIG_KEY)) {
      localStorage.setItem(CONFIG_KEY, localStorage.getItem(LEGACY_CONFIG_KEY)!);
    }
    if (!localStorage.getItem(YUQUE_PROGRESS_KEY) && localStorage.getItem(LEGACY_YUQUE_PROGRESS_KEY)) {
      localStorage.setItem(YUQUE_PROGRESS_KEY, localStorage.getItem(LEGACY_YUQUE_PROGRESS_KEY)!);
    }
  } catch {
    /* ignore */
  }
}

migrateLegacyStorageKeys();

export const STORAGE_LABEL = '浏览器 localStorage';

export const DEFAULT_APP_CONFIG: AppConfig = {
  favorites: [],
  recentPaths: [],
  settings: {
    compareMode: 'md5',
    ignorePatterns: ['.DS_Store', '.git', 'node_modules', '.stignore'],
    defaultSyncStrategy: 'primary-overwrite',
  },
  lastSession: {
    folders: [],
    compareMode: 'md5',
  },
};

export interface YuqueProgressState {
  version?: number;
  url: string;
  authMode?: string;
  namespace?: string | null;
  bookName?: string;
  bookDir?: string;
  saveDir?: string;
  total?: number;
  completedSlugs?: string[];
  failed?: { slug: string; title?: string; message: string; at?: string }[];
  docManifest?: { slug: string; title: string; dirPath: string }[];
  currentSlug?: string | null;
  status?: string;
  startedAt?: string;
  updatedAt?: string;
}

function readJson<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    if (!raw) return fallback;
    return { ...fallback, ...JSON.parse(raw) };
  } catch {
    return fallback;
  }
}

function writeJson(key: string, value: unknown) {
  localStorage.setItem(key, JSON.stringify(value));
}

export function loadAppConfig(): AppConfig {
  const raw = readJson<Partial<AppConfig>>(CONFIG_KEY, {});
  return {
    ...structuredClone(DEFAULT_APP_CONFIG),
    ...raw,
    settings: { ...DEFAULT_APP_CONFIG.settings, ...(raw.settings || {}) },
    lastSession: { ...DEFAULT_APP_CONFIG.lastSession, ...(raw.lastSession || {}) },
    favorites: Array.isArray(raw.favorites) ? raw.favorites : [],
    recentPaths: Array.isArray(raw.recentPaths) ? raw.recentPaths : [],
  };
}

export function saveAppConfigPartial(partial: Partial<AppConfig>): AppConfig {
  const current = loadAppConfig();
  const next: AppConfig = {
    ...current,
    ...partial,
    settings: partial.settings ? { ...current.settings, ...partial.settings } : current.settings,
    lastSession: partial.lastSession ? { ...current.lastSession, ...partial.lastSession } : current.lastSession,
    favorites: partial.favorites != null ? partial.favorites : current.favorites,
    recentPaths: partial.recentPaths != null ? partial.recentPaths : current.recentPaths,
  };
  writeJson(CONFIG_KEY, next);
  return next;
}

export function addRecentPath(folderPath: string): AppConfig {
  const p = String(folderPath || '').trim();
  if (!p) return loadAppConfig();
  const config = loadAppConfig();
  const list = [p, ...(config.recentPaths || []).filter((x) => x !== p)].slice(0, 20);
  return saveAppConfigPartial({ recentPaths: list });
}

export function saveFavoriteAction(
  action: 'add' | 'remove' | 'update',
  favorite: Partial<FavoriteItem> & { id?: string },
): FavoriteItem[] {
  const config = loadAppConfig();
  let favorites = [...config.favorites];

  if (action === 'add' && favorite) {
    const id = favorite.id || `fav-${Date.now()}`;
    favorites = [{ ...favorite, id } as FavoriteItem, ...favorites.filter((f) => f.id !== id)];
  } else if (action === 'remove' && favorite?.id) {
    favorites = favorites.filter((f) => f.id !== favorite.id);
  } else if (action === 'update' && favorite?.id) {
    favorites = favorites.map((f) => (f.id === favorite.id ? { ...f, ...favorite } as FavoriteItem : f));
  }

  saveAppConfigPartial({ favorites });
  return favorites;
}

export function normalizeYuqueUrlKey(input: string): string {
  const raw = String(input || '').trim();
  if (!raw) return '';
  try {
    const u = new URL(/^https?:\/\//i.test(raw) ? raw : `https://${raw}`);
    const parts = u.pathname.split('/').filter(Boolean);
    if (parts[0] === 'docs' && parts[1] === 'share') return `share:${parts[2] || ''}`;
    if (parts.length >= 2) return `book:${parts.slice(0, 2).join('/')}`;
    return u.pathname.replace(/\/+$/, '').toLowerCase();
  } catch {
    return raw.toLowerCase();
  }
}

function yuqueProgressMap(): Record<string, YuqueProgressState> {
  try {
    const raw = localStorage.getItem(YUQUE_PROGRESS_KEY);
    return raw ? JSON.parse(raw) : {};
  } catch {
    return {};
  }
}

function saveYuqueProgressMap(map: Record<string, YuqueProgressState>) {
  writeJson(YUQUE_PROGRESS_KEY, map);
}

export function yuqueProgressKey(url: string, saveDir: string): string {
  return `${String(saveDir || '').trim()}|${normalizeYuqueUrlKey(url)}`;
}

export function loadYuqueProgress(url: string, saveDir: string): YuqueProgressState | null {
  const key = yuqueProgressKey(url, saveDir);
  const map = yuqueProgressMap();
  return map[key] || null;
}

export function saveYuqueProgress(url: string, saveDir: string, progress: YuqueProgressState | null) {
  const key = yuqueProgressKey(url, saveDir);
  const map = yuqueProgressMap();
  if (progress) map[key] = progress;
  else delete map[key];
  saveYuqueProgressMap(map);
}

export function clearYuqueProgressStorage(url?: string, saveDir?: string) {
  if (url && saveDir) {
    saveYuqueProgress(url, saveDir, null);
    return;
  }
  localStorage.removeItem(YUQUE_PROGRESS_KEY);
}
