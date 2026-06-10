import { computed } from 'vue';
import { ElMessage, ElMessageBox } from 'element-plus';
import { uid } from '@/api';
import type { PathFavorite, UrlFavorite } from '@/types';
import { useConfig } from '@/composables/useConfig';
import { savePathFavoriteAction, saveUrlFavoriteAction } from '@/utils/appStorage';

function normalizePath(path: string): string {
  const t = path.trim();
  if (!t) return '';
  return t.replace(/\/+$/, '') || t;
}

function normalizeUrl(url: string): string {
  const t = url.trim();
  if (!t) return '';
  try {
    const parsed = new URL(/^https?:\/\//i.test(t) ? t : `https://${t}`);
    return parsed.href.replace(/\/$/, '');
  } catch {
    return t;
  }
}

export function usePathUrlFavorites() {
  const { config, load } = useConfig();

  const pathFavorites = computed(() => config.value?.pathFavorites || []);
  const urlFavorites = computed(() => config.value?.urlFavorites || []);

  async function reload() {
    return load();
  }

  function findPathFavorite(path: string, excludeId?: string) {
    const key = normalizePath(path);
    if (!key) return undefined;
    return pathFavorites.value.find(
      (x) => x.id !== excludeId && normalizePath(x.path) === key,
    );
  }

  function findUrlFavorite(url: string, excludeId?: string) {
    const key = normalizeUrl(url);
    if (!key) return undefined;
    return urlFavorites.value.find(
      (x) => x.id !== excludeId && normalizeUrl(x.url) === key,
    );
  }

  async function addPathFavorite(path: string, name?: string, note?: string) {
    const p = path.trim();
    if (!p) throw new Error('路径不能为空');
    if (findPathFavorite(p)) {
      throw new Error('该地址已在收藏中');
    }
    const item: PathFavorite = {
      id: uid(),
      name: name?.trim() || p.split('/').filter(Boolean).pop() || '文件夹',
      path: p,
      note: note?.trim() || undefined,
      createdAt: new Date().toISOString(),
    };
    savePathFavoriteAction('add', item);
    await load();
    return item;
  }

  async function addUrlFavorite(url: string, name?: string, note?: string) {
    const u = url.trim();
    if (!u) throw new Error('网址不能为空');
    if (findUrlFavorite(u)) {
      throw new Error('该网址已在收藏中');
    }
    let autoName = '网址';
    try {
      const parsed = new URL(/^https?:\/\//i.test(u) ? u : `https://${u}`);
      autoName = parsed.hostname.replace(/^www\./, '');
    } catch {
      autoName = u.slice(0, 40);
    }
    const item: UrlFavorite = {
      id: uid(),
      name: name?.trim() || autoName,
      url: u,
      note: note?.trim() || undefined,
      createdAt: new Date().toISOString(),
    };
    saveUrlFavoriteAction('add', item);
    await load();
    return item;
  }

  async function updatePathFavorite(item: PathFavorite) {
    if (!item.path.trim()) throw new Error('路径不能为空');
    if (findPathFavorite(item.path, item.id)) {
      throw new Error('该地址已在收藏中');
    }
    savePathFavoriteAction('update', { ...item, updatedAt: new Date().toISOString() });
    await load();
  }

  async function updateUrlFavorite(item: UrlFavorite) {
    if (!item.url.trim()) throw new Error('网址不能为空');
    if (findUrlFavorite(item.url, item.id)) {
      throw new Error('该网址已在收藏中');
    }
    saveUrlFavoriteAction('update', { ...item, updatedAt: new Date().toISOString() });
    await load();
  }

  async function removePathFavorite(id: string) {
    savePathFavoriteAction('remove', { id });
    await load();
  }

  async function removeUrlFavorite(id: string) {
    saveUrlFavoriteAction('remove', { id });
    await load();
  }

  async function promptSavePath(path: string) {
    const p = path.trim();
    if (!p) return;
    if (findPathFavorite(p)) {
      ElMessage.warning('该地址已在收藏中');
      return;
    }
    try {
      const { value } = await ElMessageBox.prompt('为这条路径起个名称（可选）', '收藏地址', {
        confirmButtonText: '收藏',
        cancelButtonText: '取消',
        inputValue: p.split('/').filter(Boolean).pop() || '',
      });
      await addPathFavorite(p, value);
      ElMessage.success('收藏成功');
    } catch (err: unknown) {
      if (err === 'cancel' || err === 'close') return;
      const msg = err instanceof Error ? err.message : '收藏失败';
      if (msg.includes('已在收藏')) ElMessage.warning(msg);
      else ElMessage.error(msg);
    }
  }

  async function promptSaveUrl(url: string) {
    const u = url.trim();
    if (!u) return;
    if (findUrlFavorite(u)) {
      ElMessage.warning('该网址已在收藏中');
      return;
    }
    try {
      const { value } = await ElMessageBox.prompt('为这个网址起个名称（可选）', '收藏网址', {
        confirmButtonText: '收藏',
        cancelButtonText: '取消',
        inputValue: '',
      });
      await addUrlFavorite(u, value);
      ElMessage.success('收藏成功');
    } catch (err: unknown) {
      if (err === 'cancel' || err === 'close') return;
      const msg = err instanceof Error ? err.message : '收藏失败';
      if (msg.includes('已在收藏')) ElMessage.warning(msg);
      else ElMessage.error(msg);
    }
  }

  function filterPathFavorites(query: string) {
    const q = query.trim().toLowerCase();
    if (!q) return pathFavorites.value;
    return pathFavorites.value.filter(
      (p) => p.name.toLowerCase().includes(q) || p.path.toLowerCase().includes(q),
    );
  }

  function filterUrlFavorites(query: string) {
    const q = query.trim().toLowerCase();
    if (!q) return urlFavorites.value;
    return urlFavorites.value.filter(
      (u) => u.name.toLowerCase().includes(q) || u.url.toLowerCase().includes(q),
    );
  }

  return {
    pathFavorites,
    urlFavorites,
    reload,
    findPathFavorite,
    findUrlFavorite,
    addPathFavorite,
    addUrlFavorite,
    updatePathFavorite,
    updateUrlFavorite,
    removePathFavorite,
    removeUrlFavorite,
    promptSavePath,
    promptSaveUrl,
    filterPathFavorites,
    filterUrlFavorites,
  };
}
