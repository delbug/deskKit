<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { ElMessage, ElMessageBox } from 'element-plus';
import {
  batchRandomizeMd5,
  exportMd5Manifest,
  formatSize,
  openFolder,
  pickFilePath,
  pickFolder,
  pickSaveFile,
  scanMd5,
  writeTextFile,
} from '@/api';
import ClearCacheButton from '@/components/ClearCacheButton.vue';
import FavoritePathInput from '@/components/FavoritePathInput.vue';
import { useConfig } from '@/composables/useConfig';
import type { Md5FileEntry, Md5ScanResult } from '@/types';

const { config, load } = useConfig();

const rootPath = ref('');
const loading = ref(false);
const scanResult = ref<Md5ScanResult | null>(null);
const search = ref('');
const selected = ref<Set<string>>(new Set());

onMounted(async () => {
  await load();
  const saved = localStorage.getItem('md5-last-path');
  if (saved) rootPath.value = saved;
});

const filteredEntries = computed(() => {
  if (!scanResult.value) return [] as Md5FileEntry[];
  const q = search.value.trim().toLowerCase();
  if (!q) return scanResult.value.entries;
  return scanResult.value.entries.filter((e) => e.relativePath.toLowerCase().includes(q));
});

const allSelected = computed(
  () =>
    filteredEntries.value.length > 0 &&
    filteredEntries.value.every((e) => selected.value.has(e.relativePath)),
);

function toggleAll(checked: boolean) {
  if (checked) {
    selected.value = new Set(filteredEntries.value.map((e) => e.relativePath));
  } else {
    selected.value = new Set();
  }
}

function toggleRow(path: string, checked: boolean) {
  const s = new Set(selected.value);
  checked ? s.add(path) : s.delete(path);
  selected.value = s;
}

function selectedRelativePaths() {
  if (selected.value.size > 0) return [...selected.value];
  return scanResult.value?.entries.map((e) => e.relativePath) ?? [];
}

async function onPickFolder() {
  const res = await pickFolder();
  if (!res.cancelled) rootPath.value = res.path;
}

async function onPickFile() {
  const res = await pickFilePath();
  if (!res.cancelled) rootPath.value = res.path;
}

async function runScan() {
  if (!rootPath.value.trim()) return ElMessage.warning('请选择文件或文件夹路径');
  localStorage.setItem('md5-last-path', rootPath.value);
  loading.value = true;
  selected.value = new Set();
  try {
    scanResult.value = await scanMd5(rootPath.value, config.value?.settings?.ignorePatterns);
    ElMessage.success(`已计算 ${scanResult.value.stats.total} 个文件的 MD5`);
  } catch (err: unknown) {
    ElMessage.error(err instanceof Error ? err.message : String(err));
  } finally {
    loading.value = false;
  }
}

async function copyMd5(md5: string) {
  try {
    await navigator.clipboard.writeText(md5);
    ElMessage.success('MD5 已复制');
  } catch {
    ElMessage.error('复制失败');
  }
}

function removeFromList() {
  if (!scanResult.value) return;
  if (!selected.value.size) return ElMessage.warning('请先勾选要从列表移除的项');
  const removeSet = new Set(selected.value);
  const entries = scanResult.value.entries.filter((e) => !removeSet.has(e.relativePath));
  scanResult.value = {
    ...scanResult.value,
    entries,
    stats: {
      total: entries.length,
      errors: entries.filter((e) => !e.md5).length,
    },
  };
  selected.value = new Set();
  ElMessage.success(`已从列表移除 ${removeSet.size} 项（文件未删除）`);
}

async function exportManifest() {
  if (!scanResult.value?.entries.length) return ElMessage.warning('请先扫描');
  try {
    const { content } = await exportMd5Manifest(scanResult.value.entries);
    const pick = await pickSaveFile();
    if (pick.cancelled || !pick.path) return;
    const path = pick.path.endsWith('.csv') ? pick.path : `${pick.path}.csv`;
    await writeTextFile(path, content);
    ElMessage.success(`已导出到 ${path}`);
  } catch (err: unknown) {
    ElMessage.error(err instanceof Error ? err.message : String(err));
  }
}

async function runRandomize(dryRun: boolean) {
  if (!scanResult.value) return;
  const paths = selected.value.size > 0 ? [...selected.value] : scanResult.value.entries.map((e) => e.relativePath);
  if (!paths.length) return ElMessage.warning('没有可修改的文件');
  const label = dryRun ? '预览随机修改' : '随机修改 MD5';
  if (!dryRun) {
    await ElMessageBox.confirm(
      `将在 ${paths.length} 个文件末尾追加随机数据以改变 MD5，原文件内容会被追加修改，此操作不可自动撤销。`,
      label,
      { type: 'warning' },
    );
  }
  loading.value = true;
  try {
    const res = await batchRandomizeMd5(scanResult.value.rootPath, paths, dryRun);
    if (dryRun) {
      ElMessage.info(`预览：将修改 ${res.modified} 个文件`);
    } else {
      ElMessage.success(`已修改 ${res.modified} 个文件的 MD5`);
      await runScan();
    }
    if (res.errors.length) {
      ElMessage.warning(res.errors.slice(0, 3).join('；'));
    }
  } catch (err: unknown) {
    ElMessage.error(err instanceof Error ? err.message : String(err));
  } finally {
    loading.value = false;
  }
}

function handleClear() {
  rootPath.value = '';
  scanResult.value = null;
  selected.value = new Set();
}
</script>

<template>
  <div class="page md5-page">
    <div class="md5-toolbar">
      <FavoritePathInput
        v-model="rootPath"
        placeholder="文件或文件夹路径"
        :show-picker="false"
        style="flex: 1; max-width: 520px"
      />
      <el-button @click="onPickFolder">选文件夹</el-button>
      <el-button @click="onPickFile">选文件</el-button>
      <el-button type="primary" :loading="loading" @click="runScan">计算 MD5</el-button>
      <el-button v-if="rootPath" @click="openFolder(rootPath)">打开位置</el-button>
      <ClearCacheButton module="md5" @cleared="handleClear" />
    </div>

    <div v-if="scanResult" class="md5-stats">
      <el-tag>{{ scanResult.stats.total }} 个文件</el-tag>
      <el-tag v-if="scanResult.stats.errors" type="warning">{{ scanResult.stats.errors }} 个读取失败</el-tag>
      <el-tag type="info">{{ scanResult.isFile ? '单文件' : '文件夹' }}</el-tag>
    </div>

    <div v-if="scanResult" class="md5-actions">
      <el-input v-model="search" placeholder="搜索路径…" clearable class="search-input" />
      <el-button :disabled="!selected.size" @click="removeFromList">
        从列表移除{{ selected.size ? ` (${selected.size})` : '' }}
      </el-button>
      <el-button @click="exportManifest">导出 CSV</el-button>
      <el-button @click="runRandomize(true)">预览随机修改</el-button>
      <el-button type="warning" @click="runRandomize(false)">
        随机修改 MD5{{ selected.size ? ` (${selected.size})` : ' (全部)' }}
      </el-button>
    </div>

    <div v-if="scanResult" class="md5-table-wrap">
      <el-table :data="filteredEntries" height="100%" size="small" stripe v-loading="loading">
        <el-table-column width="44">
          <template #header>
            <el-checkbox :model-value="allSelected" @change="toggleAll(!!$event)" />
          </template>
          <template #default="{ row }">
            <el-checkbox
              :model-value="selected.has(row.relativePath)"
              @change="toggleRow(row.relativePath, !!$event)"
            />
          </template>
        </el-table-column>
        <el-table-column prop="relativePath" label="路径" min-width="220" show-overflow-tooltip />
        <el-table-column label="大小" width="100">
          <template #default="{ row }">{{ formatSize(row.size) }}</template>
        </el-table-column>
        <el-table-column prop="md5" label="MD5" min-width="280">
          <template #default="{ row }">
            <span
              v-if="row.md5"
              class="md5-cell clickable"
              title="点击复制"
              @click="copyMd5(row.md5!)"
            >{{ row.md5 }}</span>
            <span v-else class="muted">{{ row.error || '—' }}</span>
          </template>
        </el-table-column>
      </el-table>
    </div>

    <div v-if="!scanResult && !loading" class="empty-state">
      <el-icon><Key /></el-icon>
      <p>选择文件或文件夹后点击「计算 MD5」查看哈希值；可批量随机修改文件内容以改变 MD5，或从列表移除不需要显示的项。</p>
    </div>
  </div>
</template>

<style scoped lang="scss">
.md5-page {
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;
  padding: 16px;
  gap: 12px;
}

.md5-toolbar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
}

.md5-toolbar,
.md5-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
}

.md5-stats {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
}

.search-input {
  width: 220px;
}

.md5-table-wrap {
  flex: 1;
  min-height: 200px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 10px;
  overflow: hidden;
}

.md5-cell {
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 12px;

  &.clickable {
    cursor: pointer;

    &:hover {
      color: var(--el-color-primary);
    }
  }
}

.muted {
  color: var(--text-muted);
  font-size: 12px;
}
</style>
