<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { ElMessage, ElMessageBox } from 'element-plus';
import {
  compareFolders,
  defaultFolders,
  deleteFiles,
  formatSize,
  moveFiles,
  pickFolder,
  previewSyncFolders,
  syncFolders,
  uid,
} from '@/api';
import ClearCacheButton from '@/components/ClearCacheButton.vue';
import FavoritePathInput from '@/components/FavoritePathInput.vue';
import SyncPreviewDialog from '@/components/SyncPreviewDialog.vue';
import { useConfig } from '@/composables/useConfig';
import { useFavorites } from '@/composables/useFavorites';
import { addRecentPath } from '@/utils/appStorage';
import type { CompareExtensionMode, CompareMode, CompareResult, DiffEntry, FilterType, FolderItem, SyncPreviewOperation, SyncPreviewSummary, SyncStrategy } from '@/types';
import type { FavoriteItem } from '@/types';

const router = useRouter();
const route = useRoute();
const { config, load, saveLastSession, restoreFolders, persist } = useConfig();
const { favorites, load: loadFavorites, applyFavorite, addFavorite } = useFavorites();

const folders = ref<FolderItem[]>(defaultFolders());
const compareMode = ref<CompareMode>('md5');
const filter = ref<FilterType>('diff');
const search = ref('');
const selectedExtensions = ref<string[]>([]);
const loading = ref(false);
const compareResult = ref<CompareResult | null>(null);
const selectedPaths = ref<Set<string>>(new Set());
const syncStrategy = ref<SyncStrategy>('primary-overwrite');
const syncTargetId = ref('');
const syncSourceId = ref('');
const sizeFilterEnabled = ref(false);
const minSizeValue = ref<number | undefined>(undefined);
const maxSizeValue = ref<number | undefined>(undefined);
const sizeUnit = ref<'KB' | 'MB' | 'GB'>('KB');
const extensionMode = ref<CompareExtensionMode>('none');
const compareExtensions = ref<string[]>([]);
const favName = ref('');
const previewVisible = ref(false);
const previewOps = ref<SyncPreviewOperation[]>([]);
const previewSummary = ref<SyncPreviewSummary | null>(null);
const pendingSync = ref<{ strategy: SyncStrategy; paths: string[] } | null>(null);

const primaryFolder = computed(() => folders.value.find((f) => f.isPrimary) || folders.value[0]);
const folderList = computed(() => {
  if (!compareResult.value) return folders.value;
  return folders.value.map((f) => ({ ...f, ...(compareResult.value!.folders[f.id] || {}) }));
});

const filteredEntries = computed(() => {
  if (!compareResult.value) return [] as DiffEntry[];
  let list = compareResult.value.entries;
  if (filter.value === 'diff') list = list.filter((e) => e.status !== 'identical');
  else if (filter.value === 'identical') list = list.filter((e) => e.status === 'identical');
  else if (filter.value === 'missing') list = list.filter((e) => e.status === 'missing');
  else if (filter.value === 'content-diff') list = list.filter((e) => e.status === 'content-diff');
  else if (filter.value === 'relocated') list = list.filter((e) => e.status === 'relocated');
  if (selectedExtensions.value.length) {
    const set = new Set(selectedExtensions.value.map((e) => e.toLowerCase()));
    list = list.filter((e) => set.has(getFileExtension(e.relativePath)));
  }
  const q = search.value.trim().toLowerCase();
  if (q) list = list.filter((e) => e.relativePath.toLowerCase().includes(q));
  return list;
});

function getFileExtension(relativePath: string): string {
  const name = relativePath.split('/').pop() || relativePath;
  const dot = name.lastIndexOf('.');
  if (dot <= 0 || dot === name.length - 1) return '(无后缀)';
  return name.slice(dot + 1).toLowerCase();
}

const extensionOptions = computed(() => {
  if (!compareResult.value) return [] as { ext: string; count: number }[];
  const counts = new Map<string, number>();
  for (const e of compareResult.value.entries) {
    const ext = getFileExtension(e.relativePath);
    counts.set(ext, (counts.get(ext) || 0) + 1);
  }
  return [...counts.entries()]
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0], 'zh-CN'))
    .map(([ext, count]) => ({ ext, count }));
});

const COMMON_COMPARE_EXTENSIONS = [
  'pdf', 'doc', 'docx', 'xls', 'xlsx', 'ppt', 'pptx', 'txt', 'md', 'html',
  'css', 'js', 'ts', 'json', 'png', 'jpg', 'jpeg', 'gif', 'webp', 'mp4', 'zip',
  '(无后缀)',
];

const extensionFilterEnabled = computed(() => extensionMode.value !== 'none');
const extensionFilterActive = computed(() => selectedExtensions.value.length > 0);

function normalizeCompareExtension(ext: string) {
  const s = ext.trim().replace(/^\./, '').toLowerCase();
  return s === '无后缀' ? '(无后缀)' : s;
}

function onExtensionModeChange(mode: CompareExtensionMode | string | number | boolean | undefined) {
  const next = (mode ?? 'none') as CompareExtensionMode;
  extensionMode.value = next;
  if (next === 'none') compareExtensions.value = [];
}

const selectedCount = computed(() => selectedPaths.value.size);
const allSelected = computed(
  () => filteredEntries.value.length > 0 && filteredEntries.value.every((e) => selectedPaths.value.has(e.relativePath)),
);

onMounted(async () => {
  await loadFavorites();
  await initFromConfig();
});

watch(() => route.path, async (p) => {
  if (p === '/compare') {
    await loadFavorites();
    await initFromConfig();
  }
});

function toSizeKb(value: number | undefined, unit: 'KB' | 'MB' | 'GB'): number | undefined {
  if (value == null || value <= 0 || Number.isNaN(value)) return undefined;
  if (unit === 'KB') return Math.floor(value);
  if (unit === 'MB') return Math.floor(value * 1024);
  return Math.floor(value * 1024 * 1024);
}

function resolveCompareSizeKb() {
  if (!sizeFilterEnabled.value) return { minKb: undefined, maxKb: undefined };
  return {
    minKb: toSizeKb(minSizeValue.value, sizeUnit.value),
    maxKb: toSizeKb(maxSizeValue.value, sizeUnit.value),
  };
}

async function initFromConfig() {
  await load();
  if (config.value?.settings?.compareMode) compareMode.value = config.value.settings.compareMode;

  const s = config.value?.settings;
  if (s?.compareSizeFilterEnabled != null) {
    sizeFilterEnabled.value = s.compareSizeFilterEnabled;
    sizeUnit.value = s.compareSizeUnit === 'MB' || s.compareSizeUnit === 'GB' ? s.compareSizeUnit : 'KB';
    minSizeValue.value = s.compareMinSizeValue;
    maxSizeValue.value = s.compareMaxSizeValue;
  } else if (s?.compareMinSizeKb || s?.compareMaxSizeKb) {
    sizeFilterEnabled.value = true;
    sizeUnit.value = 'KB';
    minSizeValue.value = s.compareMinSizeKb || undefined;
    maxSizeValue.value = s.compareMaxSizeKb || undefined;
  }

  if (s?.compareExtensionMode) {
    extensionMode.value = s.compareExtensionMode;
  }
  if (s?.compareExtensions?.length) {
    compareExtensions.value = [...s.compareExtensions];
  }
  const restored = restoreFolders();
  if (restored) {
    folders.value = restored;
    if (config.value?.lastSession?.compareMode) {
      compareMode.value = config.value.lastSession.compareMode;
    }
  }

  const state = window.history.state as { autoCompare?: boolean } | null;
  if (state?.autoCompare) {
    window.history.replaceState({ ...window.history.state, autoCompare: false }, '');
    const valid = folders.value.filter((f) => f.path.trim());
    if (valid.length >= 2) {
      await runCompare();
    }
  }
}

watch([folders, compareMode], () => {
  const valid = folders.value.filter((f) => f.path.trim());
  if (valid.length) saveLastSession(folders.value, compareMode.value).catch(() => {});
}, { deep: true });

function statusLabel(entry: DiffEntry) {
  if (entry.status === 'identical') return '相同';
  if (entry.status === 'content-diff') return '内容不同';
  if (entry.status === 'missing') return '部分缺失';
  if (entry.status === 'relocated') return '路径不同';
  if (entry.status.startsWith('only-')) {
    const fid = entry.status.slice(5);
    return `仅在 ${folders.value.find((f) => f.id === fid)?.label || '未知'}`;
  }
  return entry.status;
}

function statusClass(entry: DiffEntry) {
  if (entry.status === 'identical') return 'identical';
  if (entry.status === 'content-diff') return 'content-diff';
  if (entry.status === 'missing') return 'missing';
  if (entry.status === 'relocated') return 'relocated';
  return 'only';
}

function clearExtensionFilter() {
  selectedExtensions.value = [];
}

function toggleExtension(ext: string) {
  const i = selectedExtensions.value.indexOf(ext);
  if (i >= 0) selectedExtensions.value = selectedExtensions.value.filter((e) => e !== ext);
  else selectedExtensions.value = [...selectedExtensions.value, ext];
}

function folderPathText(entry: DiffEntry, folderId: string) {
  if (entry.pathsByFolder?.[folderId]) return entry.pathsByFolder[folderId];
  if (entry.presence[folderId]) return entry.relativePath;
  return null;
}

async function onPickFolder(folder: FolderItem) {
  try {
    const res = await pickFolder();
    if (res.cancelled) return;
    folder.path = res.path;
    const isDefaultLabel = !folder.label || /^文件夹\s*.+$/.test(folder.label);
    if (isDefaultLabel) folder.label = res.name;
  } catch (err: any) {
    ElMessage.error(err.message);
  }
}

function clearFolderPath(folder: FolderItem) {
  folder.path = '';
}

function clearFolderLabel(folder: FolderItem) {
  folder.label = '';
}

function addFolder() {
  folders.value.push({ id: uid(), path: '', label: `文件夹 ${folders.value.length + 1}`, isPrimary: false });
}

function removeFolder(id: string) {
  if (folders.value.length <= 2) return ElMessage.warning('至少需要 2 个文件夹');
  const removed = folders.value.find((f) => f.id === id);
  folders.value = folders.value.filter((f) => f.id !== id);
  if (removed?.isPrimary && folders.value[0]) folders.value[0].isPrimary = true;
}

function setPrimary(id: string) {
  folders.value.forEach((f) => { f.isPrimary = f.id === id; });
}

async function runCompare() {
  const valid = folders.value.filter((f) => f.path.trim());
  if (valid.length < 2) return ElMessage.warning('请为至少 2 个文件夹选择路径');
  if (extensionMode.value !== 'none' && compareExtensions.value.length === 0) {
    return ElMessage.warning('请选择至少一种文件格式');
  }
  const { minKb, maxKb } = resolveCompareSizeKb();
  if (minKb != null && maxKb != null && minKb > maxKb) {
    return ElMessage.warning('最小文件大小不能大于最大文件大小');
  }
  loading.value = true;
  selectedPaths.value = new Set();
  selectedExtensions.value = [];
  const normalizedExtensions = compareExtensions.value.map(normalizeCompareExtension);
  try {
    const res = await compareFolders(
      valid,
      compareMode.value,
      config.value?.settings?.ignorePatterns,
      minKb,
      maxKb,
      extensionMode.value,
      extensionMode.value === 'none' ? undefined : normalizedExtensions,
    );
    compareResult.value = res;
    for (const f of valid) addRecentPath(f.path);
    syncSourceId.value = res.primaryId;
    syncTargetId.value = valid.find((f) => f.id !== res.primaryId)?.id || '';
    if (config.value) {
      await persist({
        settings: {
          ...config.value.settings,
          compareSizeFilterEnabled: sizeFilterEnabled.value,
          compareMinSizeValue: minSizeValue.value,
          compareMaxSizeValue: maxSizeValue.value,
          compareSizeUnit: sizeUnit.value,
          compareMinSizeKb: minKb,
          compareMaxSizeKb: maxKb,
          compareExtensionMode: extensionMode.value,
          compareExtensions: extensionMode.value === 'none' ? [] : normalizedExtensions,
        },
      });
    }
    ElMessage.success(`对比完成，共 ${res.stats.total} 个文件`);
  } catch (err: any) {
    ElMessage.error(err.message);
  } finally {
    loading.value = false;
  }
}

async function saveAsFavorite() {
  const valid = folders.value.filter((f) => f.path.trim());
  if (valid.length < 2) return ElMessage.warning('请先为至少 2 个文件夹选择路径');
  try {
    await addFavorite({
      name: favName.value.trim() || `收藏 ${new Date().toLocaleString('zh-CN')}`,
      folders: JSON.parse(JSON.stringify(folders.value)),
      compareMode: compareMode.value,
    });
    favName.value = '';
    ElMessage.success('已收藏，可在「收藏管理」或下方列表加载');
  } catch (err: any) {
    ElMessage.error(err.message);
  }
}

async function onQuickLoadFavorite(fav: FavoriteItem) {
  await applyFavorite(fav, { navigate: false, autoCompare: false });
  await initFromConfig();
  ElMessage.success(`已加载「${fav.name}」`);
}

function toggleSelectAll(checked: boolean) {
  selectedPaths.value = checked ? new Set(filteredEntries.value.map((e) => e.relativePath)) : new Set();
}

function toggleRow(path: string, checked: boolean) {
  const next = new Set(selectedPaths.value);
  checked ? next.add(path) : next.delete(path);
  selectedPaths.value = next;
}

function getSelectedPaths() { return [...selectedPaths.value]; }

async function confirmAndRun(title: string, message: string, fn: () => Promise<void>) {
  try {
    await ElMessageBox.confirm(message, title, { type: 'warning', confirmButtonText: '确认', cancelButtonText: '取消' });
    await fn();
  } catch { /* cancel */ }
}

async function handleSync(strategy?: SyncStrategy) {
  if (!compareResult.value) return;
  const paths = getSelectedPaths();
  const strat = strategy || syncStrategy.value;
  const validFolders = folders.value.filter((f) => f.path.trim());
  if (strat === 'selected' && (!syncSourceId.value || !syncTargetId.value)) return ElMessage.warning('请选择源和目标');
  if (strat !== 'primary-overwrite' && paths.length === 0) return ElMessage.warning('请先勾选文件');

  loading.value = true;
  try {
    const preview = await previewSyncFolders({
      strategy: strat,
      folders: validFolders,
      relativePaths: paths,
      deleteExtra: strat === 'primary-overwrite' && paths.length === 0,
      sourceFolderId: syncSourceId.value,
      targetFolderId: syncTargetId.value,
    });
    previewOps.value = preview.operations;
    previewSummary.value = preview.summary;
    pendingSync.value = { strategy: strat, paths };
    previewVisible.value = true;
  } catch (err: any) {
    ElMessage.error(err.message || '预览失败');
  } finally {
    loading.value = false;
  }
}

async function executePendingSync() {
  if (!pendingSync.value) return;
  const { strategy: strat, paths } = pendingSync.value;
  const validFolders = folders.value.filter((f) => f.path.trim());
  loading.value = true;
  try {
    await syncFolders({
      strategy: strat,
      folders: validFolders,
      relativePaths: paths,
      deleteExtra: strat === 'primary-overwrite' && paths.length === 0,
      sourceFolderId: syncSourceId.value,
      targetFolderId: syncTargetId.value,
    });
    previewVisible.value = false;
    ElMessage.success('同步完成');
    await runCompare();
  } catch (err: any) {
    ElMessage.error(err.message);
  } finally {
    loading.value = false;
  }
}

async function handleDelete() {
  const paths = getSelectedPaths();
  if (!paths.length) return ElMessage.warning('请先勾选');
  await confirmAndRun('确认删除', `删除 ${paths.length} 个文件？不可恢复。`, async () => {
    loading.value = true;
    try {
      const items: { folderPath: string; relativePath: string }[] = [];
      for (const rel of paths) {
        const entry = compareResult.value?.entries.find((e) => e.relativePath === rel);
        if (!entry) continue;
        for (const f of folders.value) {
          if (entry.presence[f.id] && f.path) items.push({ folderPath: f.path, relativePath: rel });
        }
      }
      const res = await deleteFiles(items);
      ElMessage.success(`已删除 ${res.deleted.length} 项`);
      await runCompare();
    } catch (err: any) {
      ElMessage.error(err.message);
    } finally {
      loading.value = false;
    }
  });
}

async function handleMove() {
  const paths = getSelectedPaths();
  if (!paths.length || !syncSourceId.value || !syncTargetId.value || syncSourceId.value === syncTargetId.value) {
    return ElMessage.warning('请勾选文件并选择不同源/目标');
  }
  const src = folders.value.find((f) => f.id === syncSourceId.value);
  const tgt = folders.value.find((f) => f.id === syncTargetId.value);
  if (!src?.path || !tgt?.path) return;

  await confirmAndRun('确认移动', `移动 ${paths.length} 项到「${tgt.label}」`, async () => {
    loading.value = true;
    try {
      const res = await moveFiles(paths.map((rel) => ({
        fromFolderPath: src.path,
        toFolderPath: tgt.path,
        relativePath: rel,
      })));
      ElMessage.success(`已移动 ${res.moved.length} 项`);
      await runCompare();
    } catch (err: any) {
      ElMessage.error(err.message);
    } finally {
      loading.value = false;
    }
  });
}

function cellClass(entry: DiffEntry, folderId: string) {
  if (!entry.presence[folderId]) return 'cell-absent';
  if (entry.status === 'content-diff') return 'cell-diff';
  if (entry.status === 'relocated') return 'cell-relocated';
  return 'cell-present';
}

function handleClearCompare() {
  folders.value = defaultFolders();
  compareMode.value = 'md5';
  compareResult.value = null;
  selectedPaths.value = new Set();
  filter.value = 'diff';
  search.value = '';
  selectedExtensions.value = [];
  favName.value = '';
  syncStrategy.value = 'primary-overwrite';
  syncTargetId.value = '';
  syncSourceId.value = '';
}
</script>

<template>
  <div class="page compare-page">
    <div class="page-body">
      <aside class="sidebar compare-sidebar">
        <div class="sidebar-scroll">
          <section class="sidebar-section folder-section">
            <div class="section-head">
              <h3 class="section-title">对比文件夹</h3>
              <div class="section-head-actions">
                <el-dropdown
                  v-if="favorites.length"
                  trigger="click"
                  @command="(cmd: string) => {
                    if (cmd === 'manage') router.push('/favorites');
                    else {
                      const fav = favorites.find((f) => f.id === cmd);
                      if (fav) onQuickLoadFavorite(fav);
                    }
                  }"
                >
                  <el-button link type="primary" size="small">
                    <el-icon><Star /></el-icon> 加载收藏
                    <el-icon class="el-icon--right"><ArrowDown /></el-icon>
                  </el-button>
                  <template #dropdown>
                    <el-dropdown-menu>
                      <el-dropdown-item v-for="fav in favorites" :key="fav.id" :command="fav.id">
                        {{ fav.name }}（{{ fav.folders.length }} 夹）
                      </el-dropdown-item>
                      <el-dropdown-item divided command="manage">管理收藏…</el-dropdown-item>
                    </el-dropdown-menu>
                  </template>
                </el-dropdown>
                <el-button v-else link type="primary" size="small" @click="router.push('/favorites')">
                  <el-icon><Star /></el-icon> 收藏
                </el-button>
                <ClearCacheButton module="compare" @cleared="handleClearCompare" />
              </div>
            </div>

            <div class="folder-list">
              <div
                v-for="(folder, index) in folders"
                :key="folder.id"
                class="folder-card"
                :class="{ 'is-primary': folder.isPrimary }"
              >
                <div class="folder-card-index">{{ String.fromCharCode(65 + index) }}</div>
                <div class="folder-card-main">
                  <div class="folder-card-top">
                    <el-input
                      v-model="folder.label"
                      size="small"
                      class="folder-name-input"
                      placeholder="文件夹名称"
                      clearable
                      @clear="clearFolderLabel(folder)"
                    />
                    <span v-if="folder.isPrimary" class="primary-badge">主文件夹</span>
                  </div>
                  <FavoritePathInput
                    v-model="folder.path"
                    size="small"
                    placeholder="路径（可选择或从收藏选用）"
                    class="folder-path-input"
                  />
                  <div class="folder-card-actions">
                    <el-button
                      class="folder-action-btn"
                      :type="folder.isPrimary ? 'primary' : 'default'"
                      @click="setPrimary(folder.id)"
                    >
                      <el-icon><Star /></el-icon> 主
                    </el-button>
                    <el-button
                      class="folder-action-btn folder-action-btn-icon"
                      type="danger"
                      plain
                      :disabled="folders.length <= 2"
                      @click="removeFolder(folder.id)"
                    >
                      <el-icon><Delete /></el-icon>
                    </el-button>
                  </div>
                </div>
              </div>
            </div>

            <div class="sidebar-actions">
              <el-button class="sidebar-btn" @click="addFolder">
                <el-icon><Plus /></el-icon> 添加文件夹
              </el-button>
              <div class="fav-save-row">
                <el-input v-model="favName" size="small" placeholder="收藏名称（可选）" />
                <el-button type="warning" class="fav-save-btn" @click="saveAsFavorite">
                  <el-icon><Star /></el-icon> 保存
                </el-button>
              </div>
            </div>
          </section>

          <section class="sidebar-section">
            <h3 class="section-title">对比模式</h3>
            <el-radio-group v-model="compareMode" class="radio-row">
              <el-radio value="md5">MD5 内容</el-radio>
              <el-radio value="name">仅路径</el-radio>
            </el-radio-group>
            <div class="min-size-field">
              <el-checkbox v-model="sizeFilterEnabled">按文件大小筛选</el-checkbox>
              <template v-if="sizeFilterEnabled">
                <div class="min-size-row">
                  <span class="size-bound-label">最小</span>
                  <el-input-number
                    v-model="minSizeValue"
                    :min="0"
                    :step="1"
                    controls-position="right"
                    class="size-input"
                  />
                  <span class="size-bound-label">最大</span>
                  <el-input-number
                    v-model="maxSizeValue"
                    :min="0"
                    :step="1"
                    controls-position="right"
                    class="size-input"
                  />
                  <el-select v-model="sizeUnit" size="small" class="size-unit-select">
                    <el-option label="KB" value="KB" />
                    <el-option label="MB" value="MB" />
                    <el-option label="GB" value="GB" />
                  </el-select>
                </div>
                <p class="min-size-hint">最小 = 该大小以上；最大 = 该大小以下。留空表示不限制该端，单位可自由切换。</p>
              </template>
            </div>
            <div class="ext-compare-field">
              <label>文件格式范围</label>
              <el-radio-group
                :model-value="extensionMode"
                class="radio-row ext-mode-row"
                @update:model-value="onExtensionModeChange"
              >
                <el-radio value="none">不限制</el-radio>
                <el-radio value="include">仅对比</el-radio>
                <el-radio value="exclude">排除</el-radio>
              </el-radio-group>
              <el-select
                v-if="extensionFilterEnabled"
                v-model="compareExtensions"
                multiple
                collapse-tags
                collapse-tags-tooltip
                filterable
                allow-create
                default-first-option
                clearable
                placeholder="选择或输入后缀，如 pdf、jpg"
                class="sidebar-full-width ext-compare-select"
              >
                <el-option
                  v-for="ext in COMMON_COMPARE_EXTENSIONS"
                  :key="ext"
                  :label="ext.startsWith('(') ? ext : `.${ext}`"
                  :value="ext"
                />
              </el-select>
              <p v-if="extensionFilterEnabled" class="min-size-hint">
                {{ extensionMode === 'include' ? '只扫描并对比所选格式的文件。' : '跳过所选格式，对比其余文件。' }}
                「仅对比」与「排除」不可同时使用。
              </p>
            </div>
            <el-button type="primary" class="sidebar-btn sidebar-btn-primary" :loading="loading" @click="runCompare">
              <el-icon><Search /></el-icon> 开始对比
            </el-button>
          </section>

          <section class="sidebar-section">
            <h3 class="section-title">同步策略</h3>
            <el-select v-model="syncStrategy" class="sidebar-full-width">
              <el-option label="主 → 辅（覆盖）" value="primary-overwrite" />
              <el-option label="并集同步" value="union" />
              <el-option label="选中项同步" value="selected" />
            </el-select>
            <template v-if="syncStrategy === 'selected'">
              <div class="field-label">源文件夹</div>
              <el-select v-model="syncSourceId" class="sidebar-full-width field-gap">
                <el-option v-for="f in folderList" :key="f.id" :label="f.label" :value="f.id" />
              </el-select>
              <div class="field-label">目标文件夹</div>
              <el-select v-model="syncTargetId" class="sidebar-full-width">
                <el-option v-for="f in folderList" :key="f.id" :label="f.label" :value="f.id" />
              </el-select>
            </template>
            <p v-else class="sync-hint">
              主文件夹：<strong>{{ primaryFolder?.label }}</strong>
            </p>
          </section>
        </div>
      </aside>

      <div class="main-panel">
        <div v-if="compareResult" class="stats-row">
          <div class="stat-chip"><span>总计</span><strong>{{ compareResult.stats.total }}</strong></div>
          <div class="stat-chip identical"><span>相同</span><strong>{{ compareResult.stats.identical }}</strong></div>
          <div class="stat-chip diff"><span>内容不同</span><strong>{{ compareResult.stats.contentDiff }}</strong></div>
          <div class="stat-chip relocated"><span>路径不同</span><strong>{{ compareResult.stats.relocated || 0 }}</strong></div>
          <div class="stat-chip missing"><span>缺失</span><strong>{{ compareResult.stats.missing }}</strong></div>
          <div class="stat-chip"><span>已选</span><strong>{{ selectedCount }}</strong></div>
        </div>
        <div v-if="compareResult" class="compare-toolbar">
          <el-input v-model="search" placeholder="搜索路径…" clearable class="toolbar-search" />
          <el-radio-group v-model="filter" size="small" class="toolbar-filters">
            <el-radio-button value="all">全部</el-radio-button>
            <el-radio-button value="diff">差异</el-radio-button>
            <el-radio-button value="identical">相同</el-radio-button>
            <el-radio-button value="content-diff">内容不同</el-radio-button>
            <el-radio-button value="relocated">路径不同</el-radio-button>
            <el-radio-button value="missing">缺失</el-radio-button>
          </el-radio-group>
        </div>
        <div v-if="compareResult" class="ext-filter-bar">
          <div class="ext-filter-top">
            <span class="ext-filter-label">结果后缀筛选</span>
            <el-select
              v-model="selectedExtensions"
              multiple
              collapse-tags
              collapse-tags-tooltip
              filterable
              clearable
              placeholder="选择后缀，如 pdf、jpeg"
              class="ext-select"
              @clear="clearExtensionFilter"
            >
              <el-option
                v-for="item in extensionOptions"
                :key="item.ext"
                :label="item.ext.startsWith('(') ? `${item.ext}（${item.count}）` : `.${item.ext}（${item.count}）`"
                :value="item.ext"
              />
            </el-select>
            <span v-if="extensionFilterActive" class="ext-filter-meta">
              已筛 {{ filteredEntries.length }} / {{ compareResult.entries.length }}
            </span>
          </div>
          <div v-if="extensionOptions.length" class="ext-quick">
            <span class="ext-quick-label">快捷：</span>
            <el-check-tag
              v-for="item in extensionOptions.slice(0, 12)"
              :key="item.ext"
              :checked="selectedExtensions.includes(item.ext)"
              class="ext-tag"
              @click="toggleExtension(item.ext)"
            >
              {{ item.ext.startsWith('(') ? item.ext : `.${item.ext}` }}
              <small>({{ item.count }})</small>
            </el-check-tag>
          </div>
        </div>
        <div v-if="compareResult" class="diff-area">
          <div class="diff-table-wrap">
            <el-table :data="filteredEntries" height="100%" size="small" stripe v-loading="loading">
              <el-table-column width="44" fixed>
                <template #header><el-checkbox :model-value="allSelected" @change="toggleSelectAll(!!$event)" /></template>
                <template #default="{ row }">
                  <el-checkbox :model-value="selectedPaths.has(row.relativePath)" @change="toggleRow(row.relativePath, !!$event)" />
                </template>
              </el-table-column>
              <el-table-column label="状态" width="110" fixed>
                <template #default="{ row }"><span class="status-tag" :class="statusClass(row)">{{ statusLabel(row) }}</span></template>
              </el-table-column>
              <el-table-column label="相对路径" min-width="260" fixed show-overflow-tooltip>
                <template #default="{ row }"><span class="path-cell">{{ row.relativePath }}</span></template>
              </el-table-column>
              <el-table-column v-for="folder in folderList" :key="folder.id" :label="folder.label + (folder.isPrimary ? ' ★' : '')" min-width="160">
                <template #default="{ row }">
                  <div :class="cellClass(row, folder.id)" class="folder-cell">
                    <template v-if="row.presence[folder.id]">
                      <div v-if="folderPathText(row, folder.id)" class="folder-cell-path" :title="folderPathText(row, folder.id)!">
                        {{ folderPathText(row, folder.id) }}
                      </div>
                      <div>{{ formatSize(row.sizes[folder.id]) }}</div>
                      <div v-if="compareMode==='md5' && row.md5s[folder.id]" class="folder-cell-md5">{{ row.md5s[folder.id]?.slice(0,8) }}…</div>
                    </template>
                    <template v-else>—</template>
                  </div>
                </template>
              </el-table-column>
            </el-table>
          </div>
        </div>
        <div v-else class="empty-state">
          <el-icon><DocumentCopy /></el-icon>
          <p>添加文件夹并开始对比</p>
          <p class="hint">刷新页面后会自动恢复上次的文件夹配置</p>
        </div>
        <footer v-if="compareResult" class="compare-action-bar">
          <div class="action-group">
            <el-button type="primary" :loading="loading" @click="handleSync('primary-overwrite')">主 → 全部辅</el-button>
            <el-button :loading="loading" @click="handleSync('union')">并集同步</el-button>
            <el-button :loading="loading" @click="handleSync('selected')">同步选中</el-button>
          </div>
          <div class="action-group">
            <el-button :loading="loading" @click="handleMove">移动选中</el-button>
            <el-button type="danger" :loading="loading" @click="handleDelete">删除选中</el-button>
          </div>
          <span class="action-meta">{{ filteredEntries.length }} / {{ compareResult.entries.length }}</span>
        </footer>
      </div>
    </div>

    <SyncPreviewDialog
      v-model:visible="previewVisible"
      :operations="previewOps"
      :summary="previewSummary"
      :loading="loading"
      @confirm="executePendingSync"
    />
  </div>
</template>

<style scoped lang="scss">
.compare-page .page-body {
  display: flex;
  flex: 1;
  min-height: 0;
  height: calc(100vh - 56px);
}

.compare-sidebar {
  width: 360px;
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.compare-sidebar :deep(.sidebar-section) {
  padding: 16px;
}

.compare-sidebar :deep(.sidebar-section + .sidebar-section) {
  border-top: 1px solid var(--border);
}

.sidebar-scroll {
  flex: 1;
  overflow-y: auto;
  min-height: 0;
}

.section-title {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
  color: var(--text-muted);
  letter-spacing: 0.04em;
}

.section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 12px;
}

.section-head-actions {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-wrap: wrap;
}

.folder-section {
  padding-bottom: 12px;
}

.folder-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
  max-height: none;
}

.folder-card {
  display: flex;
  align-items: stretch;
  gap: 10px;
  padding: 12px;
  background: var(--surface-2);
  border: 1px solid var(--border);
  border-radius: 10px;
  transition: border-color 0.15s, box-shadow 0.15s;

  &.is-primary {
    border-color: rgba(59, 130, 246, 0.55);
    box-shadow: inset 3px 0 0 var(--primary);
  }
}

.folder-card-index {
  flex-shrink: 0;
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 8px;
  background: var(--bg);
  border: 1px solid var(--border);
  font-size: 12px;
  font-weight: 700;
  color: var(--text-muted);
  margin-top: 2px;
}

.folder-card-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.folder-card-top {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.primary-badge {
  flex-shrink: 0;
  white-space: nowrap;
  font-size: 10px;
}

.folder-name-input {
  flex: 1;
  min-width: 0;
}

.folder-name-input,
.folder-path-input {
  width: 100%;
}

.folder-path-input :deep(.el-input__inner) {
  font-size: 11px;
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
}

.folder-card-actions {
  display: grid;
  grid-template-columns: 1fr 1fr auto;
  gap: 8px;
  align-items: stretch;
}

.folder-action-btn {
  margin: 0 !important;
  width: 100%;
}

.folder-action-btn-icon {
  width: 32px;
  min-width: 32px;
  padding: 0;
}

.sidebar-actions {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin-top: 14px;
}

.sidebar-btn {
  width: 100%;
  margin: 0 !important;
}

.sidebar-btn-primary {
  margin-top: 4px !important;
}

.fav-save-row {
  display: flex;
  align-items: stretch;
  gap: 8px;

  .el-input {
    flex: 1;
    min-width: 0;
  }
}

.fav-save-btn {
  margin: 0 !important;
  flex-shrink: 0;
}

.radio-row {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 8px;
  width: 100%;
  margin-bottom: 12px;
}

.field-label {
  font-size: 12px;
  color: var(--text-muted);
  margin: 10px 0 6px;
}

.sidebar-full-width {
  width: 100%;
}

.compare-toolbar {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 12px;
  padding: 14px 16px;
  background: var(--surface);
  border-bottom: 1px solid var(--border);
}

.toolbar-filters {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;

  :deep(.el-radio-button) {
    margin: 0;
  }

  :deep(.el-radio-button__inner) {
    padding: 7px 14px;
    border-radius: 6px !important;
  }
}

.compare-action-bar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 14px 20px;
  padding: 14px 18px;
  background: var(--surface);
  border-top: 1px solid var(--border);
  flex-shrink: 0;
}

.action-group {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;

  .el-button {
    margin: 0 !important;
  }
}

.field-gap {
  margin-bottom: 4px;
}

.sync-hint {
  margin: 10px 0 0;
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-muted);

  strong {
    color: var(--text);
    font-weight: 600;
  }
}

.min-size-field {
  margin-top: 12px;

  label {
    display: block;
    margin-bottom: 6px;
    font-size: 12px;
    color: var(--text-muted);
  }
}

.min-size-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  margin-bottom: 6px;
}

.size-bound-label {
  font-size: 12px;
  color: var(--text-muted);
  flex-shrink: 0;
}

.size-input {
  width: 120px;
}

.size-unit-select {
  width: 72px;
}

.min-size-unit {
  font-size: 12px;
  color: var(--text-muted);
}

.min-size-hint {
  margin: 8px 0 12px;
  font-size: 11px;
  line-height: 1.5;
  color: var(--text-muted);
}

.ext-compare-field {
  margin-top: 4px;

  label {
    display: block;
    margin-bottom: 6px;
    font-size: 12px;
    color: var(--text-muted);
  }
}

.ext-mode-row {
  margin-bottom: 8px;
}

.ext-compare-select {
  width: 100%;
}

.action-meta {
  margin-left: auto;
  padding-left: 8px;
  font-size: 12px;
  color: var(--text-muted);
  white-space: nowrap;
}

.hint {
  font-size: 13px;
  color: var(--text-muted);
}

.folder-cell {
  font-size: 12px;
}

.folder-cell-path {
  font-size: 10px;
  opacity: 0.85;
  word-break: break-all;
  margin-bottom: 2px;
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
}

.folder-cell-md5 {
  opacity: 0.6;
  font-size: 10px;
}

.toolbar-search {
  width: 220px;
  flex-shrink: 0;
}

.ext-filter-bar {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 12px 16px;
  background: var(--bg);
  border-bottom: 1px solid var(--border);
}

.ext-filter-top {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 12px;
}

.ext-filter-label {
  font-size: 12px;
  color: var(--text-muted);
  flex-shrink: 0;
  min-width: 56px;
}

.ext-select {
  width: min(320px, 100%);
  min-width: 200px;
  flex: 1;
}

.ext-quick {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  padding-top: 2px;
}

.ext-quick-label {
  font-size: 12px;
  color: var(--text-muted);
  flex-shrink: 0;
}

.ext-tag {
  cursor: pointer;
  font-size: 12px;

  small {
    opacity: 0.75;
  }
}

.ext-filter-meta {
  font-size: 12px;
  color: var(--text-muted);
  flex-shrink: 0;
}
</style>
