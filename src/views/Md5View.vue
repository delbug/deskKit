<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { ElMessage, ElMessageBox } from 'element-plus';
import {
  batchRenameByMd5,
  exportMd5Manifest,
  formatSize,
  openFolder,
  pickFilePath,
  pickFolder,
  pickSaveFile,
  scanMd5,
  verifyMd5Manifest,
  writeTextFile,
} from '@/api';
import ClearCacheButton from '@/components/ClearCacheButton.vue';
import FavoritePathInput from '@/components/FavoritePathInput.vue';
import { useConfig } from '@/composables/useConfig';
import type { Md5FileEntry, Md5RenameMode, Md5ScanResult, Md5VerifyResult } from '@/types';

const { config, load } = useConfig();

const rootPath = ref('');
const minSizeKb = ref(0);
const loading = ref(false);
const scanResult = ref<Md5ScanResult | null>(null);
const search = ref('');
const selected = ref<Set<string>>(new Set());
const renameMode = ref<Md5RenameMode>('suffix');
const manifestText = ref('');
const verifyResult = ref<Md5VerifyResult | null>(null);

onMounted(async () => {
  await load();
  const saved = localStorage.getItem('md5-last-path');
  if (saved) rootPath.value = saved;
  if (config.value?.settings?.compareMinSizeKb) {
    minSizeKb.value = config.value.settings.compareMinSizeKb;
  }
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

function selectedEntries() {
  if (!scanResult.value) return [] as Md5FileEntry[];
  if (selected.value.size === 0) return scanResult.value.entries;
  return scanResult.value.entries.filter((e) => selected.value.has(e.relativePath));
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
  verifyResult.value = null;
  try {
    scanResult.value = await scanMd5(
      rootPath.value,
      minSizeKb.value,
      config.value?.settings?.ignorePatterns,
    );
    ElMessage.success(`已计算 ${scanResult.value.stats.total} 个文件的 MD5`);
  } catch (err: unknown) {
    ElMessage.error(err instanceof Error ? err.message : String(err));
  } finally {
    loading.value = false;
  }
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

async function runVerify() {
  if (!scanResult.value?.entries.length) return ElMessage.warning('请先扫描');
  if (!manifestText.value.trim()) return ElMessage.warning('请粘贴 MD5 清单');
  try {
    verifyResult.value = await verifyMd5Manifest(scanResult.value.entries, manifestText.value);
    ElMessage.success(
      `校验完成：匹配 ${verifyResult.value.matched}，不一致 ${verifyResult.value.mismatched}，缺失 ${verifyResult.value.missing}`,
    );
  } catch (err: unknown) {
    ElMessage.error(err instanceof Error ? err.message : String(err));
  }
}

async function runRename(dryRun: boolean) {
  if (!scanResult.value) return;
  const entries = selectedEntries();
  if (!entries.length) return ElMessage.warning('没有可重命名的文件');
  const modeLabel =
    renameMode.value === 'prefix'
      ? 'MD5 前缀'
      : renameMode.value === 'suffix'
        ? 'MD5 后缀'
        : '仅 MD5 文件名';
  if (!dryRun) {
    await ElMessageBox.confirm(
      `将按「${modeLabel}」重命名 ${entries.length} 个文件，此操作不可自动撤销。`,
      '确认重命名',
      { type: 'warning' },
    );
  }
  loading.value = true;
  try {
    const res = await batchRenameByMd5(scanResult.value.rootPath, entries, renameMode.value, dryRun);
    if (dryRun) {
      ElMessage.info(`预览：将重命名 ${res.renamed} 个，跳过 ${res.skipped} 个`);
    } else {
      ElMessage.success(`已重命名 ${res.renamed} 个文件`);
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
  verifyResult.value = null;
  manifestText.value = '';
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
      <div class="min-size-inline">
        <span>≥</span>
        <el-input-number v-model="minSizeKb" :min="0" :max="999999" :step="1" controls-position="right" size="small" />
        <span>KB</span>
      </div>
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
      <el-button @click="exportManifest">导出 CSV 清单</el-button>
      <el-select v-model="renameMode" style="width: 160px">
        <el-option label="MD5 前缀" value="prefix" />
        <el-option label="MD5 后缀" value="suffix" />
        <el-option label="仅 MD5 作文件名" value="hashOnly" />
      </el-select>
      <el-button @click="runRename(true)">预览重命名</el-button>
      <el-button type="warning" :disabled="!selected.size && !scanResult.entries.length" @click="runRename(false)">
        按 MD5 重命名{{ selected.size ? ` (${selected.size})` : ' (全部)' }}
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
        <el-table-column prop="relativePath" label="路径" min-width="240" show-overflow-tooltip />
        <el-table-column label="大小" width="100">
          <template #default="{ row }">{{ formatSize(row.size) }}</template>
        </el-table-column>
        <el-table-column prop="md5" label="MD5" min-width="280">
          <template #default="{ row }">
            <span v-if="row.md5" class="md5-cell">{{ row.md5 }}</span>
            <span v-else class="muted">{{ row.error || '—' }}</span>
          </template>
        </el-table-column>
      </el-table>
    </div>

    <div v-if="scanResult" class="verify-panel">
      <h3>MD5 清单校验</h3>
      <p class="hint">每行格式：<code>相对路径,md5</code> 或 <code>md5 相对路径</code>，可与导出的 CSV 对照。</p>
      <el-input v-model="manifestText" type="textarea" :rows="4" placeholder="粘贴 MD5 清单…" />
      <div class="verify-actions">
        <el-button @click="runVerify">校验</el-button>
        <span v-if="verifyResult" class="verify-summary">
          匹配 {{ verifyResult.matched }} · 不一致 {{ verifyResult.mismatched }} · 缺失 {{ verifyResult.missing }}
        </span>
      </div>
    </div>

    <div v-if="!scanResult && !loading" class="empty-state">
      <el-icon><Key /></el-icon>
      <p>选择文件或文件夹，批量查看 MD5；支持导出清单、校验与按 MD5 重命名。</p>
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

.min-size-inline {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--text-muted);
}

.md5-stats,
.md5-actions {
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
}

.muted {
  color: var(--text-muted);
  font-size: 12px;
}

.verify-panel {
  padding: 14px 16px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 10px;

  h3 {
    margin: 0 0 8px;
    font-size: 14px;
  }

  .hint {
    margin: 0 0 8px;
    font-size: 12px;
    color: var(--text-muted);
  }
}

.verify-actions {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-top: 8px;
}

.verify-summary {
  font-size: 12px;
  color: var(--text-muted);
}
</style>
