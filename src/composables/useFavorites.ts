import { computed } from 'vue';
import { useRouter } from 'vue-router';
import { uid } from '@/api';
import type { CompareMode, FavoriteItem, FolderItem } from '@/types';
import { useConfig } from '@/composables/useConfig';

export function defaultFavoriteFolders(): FolderItem[] {
  return [
    { id: uid(), path: '', label: '文件夹 A', isPrimary: true },
    { id: uid(), path: '', label: '文件夹 B', isPrimary: false },
  ];
}

export function useFavorites() {
  const router = useRouter();
  const { config, load, persist, saveFavorite } = useConfig();

  const favorites = computed(() => config.value?.favorites || []);

  async function applyFavorite(
    fav: FavoriteItem,
    options?: { navigate?: boolean; autoCompare?: boolean },
  ) {
    const folders = fav.folders.map((f) => ({ ...f, id: f.id || uid() }));
    const compareMode: CompareMode =
      fav.compareMode || config.value?.lastSession?.compareMode || config.value?.settings?.compareMode || 'md5';

    await persist({
      lastSession: { folders, compareMode },
    });

    if (options?.navigate !== false) {
      await router.push({
        path: '/compare',
        state: { autoCompare: !!options?.autoCompare },
      });
    }
    return load();
  }

  async function addFavorite(input: {
    name: string;
    folders: FolderItem[];
    note?: string;
    compareMode?: CompareMode;
  }) {
    const valid = input.folders.filter((f) => f.path.trim());
    if (valid.length < 2) {
      throw new Error('请为至少 2 个文件夹填写路径');
    }
    const item: FavoriteItem = {
      id: uid(),
      name: input.name.trim() || `收藏 ${new Date().toLocaleString('zh-CN')}`,
      folders: input.folders.map((f) => ({ ...f, id: f.id || uid() })),
      note: input.note?.trim() || undefined,
      compareMode: input.compareMode,
      createdAt: new Date().toISOString(),
    };
    await saveFavorite('add', item);
    await load();
    return item;
  }

  async function updateFavorite(fav: FavoriteItem) {
    const valid = fav.folders.filter((f) => f.path.trim());
    if (valid.length < 2) {
      throw new Error('请为至少 2 个文件夹填写路径');
    }
    await saveFavorite('update', { ...fav, updatedAt: new Date().toISOString() });
    await load();
  }

  async function removeFavorite(id: string) {
    await saveFavorite('remove', { id });
    await load();
  }

  return {
    config,
    favorites,
    load,
    applyFavorite,
    addFavorite,
    updateFavorite,
    removeFavorite,
  };
}
