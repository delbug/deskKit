<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue';
import { ElMessage, ElMessageBox } from 'element-plus';
import {
  deleteFiles,
  executeRename,
  findFiles,
  formatSize,
  moveFiles,
  openFolder,
  pickFolder,
  pickSaveFile,
  previewRenameSelected,
  writeTextFile,
} from '@/api';
import ClearCacheButton from '@/components/ClearCacheButton.vue';
import FavoritePathInput from '@/components/FavoritePathInput.vue';
import type { FindFileEntry, FindFilesMatchMode, RenamePlanItem, RenameRules } from '@/types';

const rootPath = ref('');
const matchMode = ref<FindFilesMatchMode>('name');
const pattern = ref('');
const caseSensitive = ref(false);
const matchFullPath = ref(false);
const sizeFilterEnabled = ref(false);
const minSizeValue = ref<number | undefined>(undefined);
const maxSizeValue = ref<number | undefined>(undefined);
const sizeUnit = ref<'KB' | 'MB' | 'GB'>('KB');
const listFilter = ref('');
const loading = ref(false);
const files = ref<FindFileEntry[]>([]);
const stats = ref({ count: 0, totalBytes: 0 });
const selected = ref<Set<string>>(new Set());

const moveDialogVisible = ref(false);
const moveTargetDir = ref('');
const preserveStructure = ref(true);

const renameDialogVisible = ref(false);
const renameLoading = ref(false);
const renamePlan = ref<RenamePlanItem[]>([]);
const renameActiveTab = ref('basic');
const removeText = ref('');
const renameRules = reactive<RenameRules>({
  prefix: '',
  suffix: '',
  replaceFrom: '',
  replaceTo: '',
  removePatterns: [],
  sequence: {
    enabled: false,
    position: 'suffix',
    insertIndex: 0,
    start: 1,
    step: 1,
    padWidth: 2,
    separator: '_',
  },
  insert: {
    enabled: false,
    index: 0,
    content: '',
    useSequence: false,
  },
  deleteAt: {
    enabled: false,
    start: 2,
    count: 2,
  },
});

const patternPlaceholder = computed(() => {
  if (matchMode.value === 'name') return '关键词或通配符，如 report 或 *.tmp';
  if (matchMode.value === 'suffix') return '后缀，如 .bak 或 _copy';
  if (matchMode.value === 'extension') return '扩展名，如 md, txt, pdf（可逗号分隔）';
  return '正则表达式，如 ^IMG_\\d+\\.jpg$';
});

const filteredFiles = computed(() => {
  const q = listFilter.value.trim().toLowerCase();
  if (!q) return files.value;
  return files.value.filter((f) => f.relativePath.toLowerCase().includes(q));
});

const selectedCount = computed(() => selected.value.size);
const renameReady = computed(() => renamePlan.value.filter((p) => p.status === 'ready'));

const deleteAtPreview = computed(() => {
  const keep = renameRules.deleteAt?.start ?? 0;
  const count = renameRules.deleteAt?.count ?? 0;
  const sample = '123456';
  if (!count || count <= 0) return sample;
  if (keep >= sample.length) return sample;
  return sample.slice(0, keep) + sample.slice(keep + count);
});

function toBytes(value: number | undefined, unit: 'KB' | 'MB' | 'GB'): number | undefined {
  if (value == null || value <= 0 || Number.isNaN(value)) return undefined;
  const mult = unit === 'KB' ? 1024 : unit === 'MB' ? 1024 ** 2 : 1024 ** 3;
  return Math.floor(value * mult);
}

function resetRenameRules() {
  renameRules.prefix = '';
  renameRules.suffix = '';
  renameRules.replaceFrom = '';
  renameRules.replaceTo = '';
  renameRules.removePatterns = [];
  if (renameRules.sequence) {
    renameRules.sequence.enabled = false;
    renameRules.sequence.position = 'suffix';
    renameRules.sequence.start = 1;
    renameRules.sequence.step = 1;
    renameRules.sequence.padWidth = 2;
    renameRules.sequence.separator = '_';
  }
  if (renameRules.insert) {
    renameRules.insert.enabled = false;
    renameRules.insert.index = 0;
    renameRules.insert.content = '';
    renameRules.insert.useSequence = false;
  }
  if (renameRules.deleteAt) {
    renameRules.deleteAt.enabled = false;
    renameRules.deleteAt.start = 2;
    renameRules.deleteAt.count = 2;
  }
  removeText.value = '';
  renameActiveTab.value = 'basic';
}

function buildRenamePayload(): RenameRules {
  return {
    ...renameRules,
    sequence: renameRules.sequence ? { ...renameRules.sequence } : undefined,
    insert: renameRules.insert ? { ...renameRules.insert } : undefined,
    deleteAt: renameRules.deleteAt ? { ...renameRules.deleteAt } : undefined,
    removePatterns: removeText.value.split(/[/、，,\n]+/).map((s) => s.trim()).filter(Boolean),
  };
}

function buildExportContent(list: FindFileEntry[], format: 'txt' | 'csv' | 'json'): string {
  if (format === 'txt') {
    return list.map((f) => f.relativePath).join('\n');
  }
  if (format === 'csv') {
    const esc = (s: string) => `"${s.replace(/"/g, '""')}"`;
    const header = 'relativePath,absolutePath,sizeBytes,mtime\n';
    const rows = list
      .map((f) => `${esc(f.relativePath)},${esc(f.absolutePath)},${f.size},${f.mtime || ''}`)
      .join('\n');
    return header + rows;
  }
  return JSON.stringify(
    {
      rootPath: rootPath.value,
      exportedAt: new Date().toISOString(),
      count: list.length,
      files: list,
    },
    null,
    2,
  );
}

function exportFileName(format: 'txt' | 'csv' | 'json') {
  const base = rootPath.value.split('/').filter(Boolean).pop() || 'find-files';
  const safe = base.replace(/[^\w\u4e00-\u9fff-]+/g, '_');
  return `${safe}-files.${format === 'txt' ? 'txt' : format}`;
}

onMounted(() => {
  const savedPath = localStorage.getItem('find-files-last-path');
  if (savedPath) rootPath.value = savedPath;
  const savedMode = localStorage.getItem('find-files-match-mode');
  if (savedMode === 'name' || savedMode === 'suffix' || savedMode === 'extension' || savedMode === 'regex') {
    matchMode.value = savedMode;
  }
  const savedPattern = localStorage.getItem('find-files-pattern');
  if (savedPattern) pattern.value = savedPattern;
  sizeFilterEnabled.value = localStorage.getItem('find-files-size-enabled') === '1';
  const savedUnit = localStorage.getItem('find-files-size-unit');
  if (savedUnit === 'KB' || savedUnit === 'MB' || savedUnit === 'GB') sizeUnit.value = savedUnit;
  const savedMin = localStorage.getItem('find-files-min-size');
  if (savedMin) minSizeValue.value = Number(savedMin) || undefined;
  const savedMax = localStorage.getItem('find-files-max-size');
  if (savedMax) maxSizeValue.value = Number(savedMax) || undefined;
});

function persistSettings() {
  localStorage.setItem('find-files-last-path', rootPath.value);
  localStorage.setItem('find-files-match-mode', matchMode.value);
  localStorage.setItem('find-files-pattern', pattern.value);
  localStorage.setItem('find-files-size-enabled', sizeFilterEnabled.value ? '1' : '0');
  localStorage.setItem('find-files-size-unit', sizeUnit.value);
  if (minSizeValue.value != null) localStorage.setItem('find-files-min-size', String(minSizeValue.value));
  else localStorage.removeItem('find-files-min-size');
  if (maxSizeValue.value != null) localStorage.setItem('find-files-max-size', String(maxSizeValue.value));
  else localStorage.removeItem('find-files-max-size');
}

async function scan() {
  if (!rootPath.value) return ElMessage.warning('请选择要搜索的文件夹');
  if (!pattern.value.trim() && matchMode.value !== 'extension') {
    return ElMessage.warning('请填写匹配内容');
  }
  persistSettings();
  loading.value = true;
  selected.value = new Set();
  const minSize = sizeFilterEnabled.value ? toBytes(minSizeValue.value, sizeUnit.value) : undefined;
  const maxSize = sizeFilterEnabled.value ? toBytes(maxSizeValue.value, sizeUnit.value) : undefined;
  if (sizeFilterEnabled.value && minSize != null && maxSize != null && minSize > maxSize) {
    loading.value = false;
    return ElMessage.warning('最小文件大小不能大于最大文件大小');
  }
  try {
    const res = await findFiles({
      rootPath: rootPath.value,
      matchMode: matchMode.value,
      pattern: pattern.value.trim(),
      caseSensitive: caseSensitive.value,
      matchFullPath: matchFullPath.value,
      minSize,
      maxSize,
    });
    files.value = res.files;
    stats.value = res.stats;
    ElMessage.success(`找到 ${res.stats.count} 个文件`);
  } catch (err: any) {
    ElMessage.error(err.message || '搜索失败');
  } finally {
    loading.value = false;
  }
}

function toggleFile(path: string, checked: boolean) {
  const s = new Set(selected.value);
  if (checked) s.add(path);
  else s.delete(path);
  selected.value = s;
}

function toggleAll(checked: boolean) {
  selected.value = checked
    ? new Set(filteredFiles.value.map((f) => f.relativePath))
    : new Set();
}

function formatMtime(ts: number) {
  if (!ts) return '—';
  return new Date(ts).toLocaleString();
}

function selectedPaths(): string[] {
  return [...selected.value];
}

async function deleteSelected() {
  const paths = selectedPaths();
  if (!paths.length) return ElMessage.warning('请勾选要删除的文件');
  try {
    await ElMessageBox.confirm(`确认删除选中的 ${paths.length} 个文件？此操作不可恢复。`, '批量删除', {
      type: 'warning',
      confirmButtonText: '确认删除',
    });
  } catch {
    return;
  }
  loading.value = true;
  try {
    const res = await deleteFiles(paths.map((p) => ({ folderPath: rootPath.value, relativePath: p })));
    ElMessage.success(`已删除 ${res.deleted.length} 项${res.errors.length ? `，失败 ${res.errors.length}` : ''}`);
    await scan();
  } catch (err: any) {
    ElMessage.error(err.message || '删除失败');
  } finally {
    loading.value = false;
  }
}

function openMoveDialog() {
  if (!selectedCount.value) return ElMessage.warning('请勾选要移动的文件');
  moveTargetDir.value = '';
  preserveStructure.value = true;
  moveDialogVisible.value = true;
}

async function pickMoveTarget() {
  const res = await pickFolder();
  if (!res.cancelled) moveTargetDir.value = res.path;
}

async function confirmMove() {
  if (!moveTargetDir.value) return ElMessage.warning('请选择目标文件夹');
  const paths = selectedPaths();
  loading.value = true;
  try {
    const items = paths.map((relativePath) => {
      const fileName = relativePath.split('/').pop() || relativePath;
      return {
        fromFolderPath: rootPath.value,
        toFolderPath: moveTargetDir.value,
        relativePath,
        targetRelativePath: preserveStructure.value ? relativePath : fileName,
      };
    });
    const res = await moveFiles(items);
    moveDialogVisible.value = false;
    ElMessage.success(`已移动 ${res.moved.length} 项${res.errors.length ? `，失败 ${res.errors.length}` : ''}`);
    await scan();
  } catch (err: any) {
    ElMessage.error(err.message || '移动失败');
  } finally {
    loading.value = false;
  }
}

function openRenameDialog() {
  if (!selectedCount.value) return ElMessage.warning('请勾选要重命名的文件');
  resetRenameRules();
  renamePlan.value = [];
  renameDialogVisible.value = true;
}

async function exportResults(scope: 'all' | 'selected', format: 'txt' | 'csv' | 'json') {
  const list =
    scope === 'selected'
      ? files.value.filter((f) => selected.value.has(f.relativePath))
      : files.value;
  if (!list.length) {
    return ElMessage.warning(scope === 'selected' ? '请先勾选要导出的文件' : '没有可导出的结果');
  }
  const picked = await pickSaveFile(exportFileName(format));
  if (picked.cancelled || !picked.path) return;
  try {
    await writeTextFile(picked.path, buildExportContent(list, format));
    ElMessage.success(`已导出 ${list.length} 条记录`);
  } catch (err: any) {
    ElMessage.error(err.message || '导出失败');
  }
}

async function previewRenamePlan() {
  if (!rootPath.value) return;
  renameLoading.value = true;
  try {
    const res = await previewRenameSelected({
      rootPath: rootPath.value,
      relativePaths: selectedPaths(),
      rules: buildRenamePayload(),
    });
    renamePlan.value = res.plan.filter((p) => p.status !== 'unchanged');
    if (!renameReady.value.length) {
      ElMessage.warning('没有可重命名的项，请调整规则');
    } else {
      ElMessage.success(`预览：${renameReady.value.length} 项将重命名`);
    }
  } catch (err: any) {
    ElMessage.error(err.message || '预览失败');
  } finally {
    renameLoading.value = false;
  }
}

async function applyRename() {
  if (!renameReady.value.length) {
    return ElMessage.warning('请先预览并确认有可执行项');
  }
  try {
    await ElMessageBox.confirm(`确认重命名 ${renameReady.value.length} 个文件？`, '批量重命名', {
      type: 'warning',
    });
  } catch {
    return;
  }
  renameLoading.value = true;
  try {
    const res = await executeRename(rootPath.value, renameReady.value);
    renameDialogVisible.value = false;
    ElMessage.success(`成功 ${res.renamed.length} 项${res.errors.length ? `，失败 ${res.errors.length}` : ''}`);
    await scan();
  } catch (err: any) {
    ElMessage.error(err.message || '重命名失败');
  } finally {
    renameLoading.value = false;
  }
}

function handleClearFindFiles() {
  rootPath.value = '';
  pattern.value = '';
  sizeFilterEnabled.value = false;
  minSizeValue.value = undefined;
  maxSizeValue.value = undefined;
  files.value = [];
  stats.value = { count: 0, totalBytes: 0 };
  selected.value = new Set();
  listFilter.value = '';
}
</script>

<template>
  <div class="page find-page">
    <div class="find-header">
      <div>
        <h2>查找文件</h2>
        <p class="hint">按名称、后缀、扩展名或正则表达式搜索，支持批量重命名、删除、移动。</p>
      </div>
      <ClearCacheButton module="findFiles" @cleared="handleClearFindFiles" />
    </div>

    <div class="find-toolbar">
      <FavoritePathInput v-model="rootPath" placeholder="搜索文件夹" style="flex:1;max-width:480px" />
      <el-button v-if="rootPath" @click="openFolder(rootPath)">在 Finder 打开</el-button>
    </div>

    <div class="search-card">
      <div class="search-row">
        <label>匹配方式</label>
        <el-radio-group v-model="matchMode">
          <el-radio value="name">名称 / 通配符</el-radio>
          <el-radio value="suffix">后缀</el-radio>
          <el-radio value="extension">扩展名 / 格式</el-radio>
          <el-radio value="regex">正则表达式</el-radio>
        </el-radio-group>
      </div>
      <div class="search-row">
        <label>匹配内容</label>
        <el-input v-model="pattern" :placeholder="patternPlaceholder" clearable @keyup.enter="scan" />
        <el-button type="primary" :loading="loading" @click="scan">搜索</el-button>
      </div>
      <div class="search-options">
        <el-checkbox v-model="caseSensitive">区分大小写</el-checkbox>
        <el-checkbox v-if="matchMode === 'regex'" v-model="matchFullPath">匹配完整相对路径</el-checkbox>
      </div>
      <div class="size-filter-row">
        <el-checkbox v-model="sizeFilterEnabled">按文件大小筛选</el-checkbox>
        <template v-if="sizeFilterEnabled">
          <span class="size-label">最小</span>
          <el-input-number v-model="minSizeValue" :min="0" :step="1" controls-position="right" class="size-input" />
          <span class="size-label">最大</span>
          <el-input-number v-model="maxSizeValue" :min="0" :step="1" controls-position="right" class="size-input" />
          <el-select v-model="sizeUnit" size="small" class="size-unit">
            <el-option label="KB" value="KB" />
            <el-option label="MB" value="MB" />
            <el-option label="GB" value="GB" />
          </el-select>
          <span class="mode-tip inline">留空表示不限制该端</span>
        </template>
      </div>
      <p class="mode-tip">
        <template v-if="matchMode === 'name'">支持包含关键词，或使用 <code>*</code>、<code>?</code> 通配符（如 <code>*.log</code>）。</template>
        <template v-else-if="matchMode === 'suffix'">匹配文件名末尾，例如 <code>.bak</code>、<code>_final</code>。</template>
        <template v-else-if="matchMode === 'extension'">按文件类型筛选，例如 <code>md, html, png</code>。</template>
        <template v-else>正则默认匹配文件名；勾选「匹配完整相对路径」可匹配如 <code>backup/.*\\.tmp$</code>。</template>
      </p>
    </div>

    <div v-if="files.length" class="result-bar">
      <el-tag>{{ stats.count }} 个文件</el-tag>
      <el-tag type="info">合计 {{ formatSize(stats.totalBytes) }}</el-tag>
      <el-tag v-if="selectedCount" type="warning">已选 {{ selectedCount }}</el-tag>
      <div class="result-actions">
        <el-checkbox
          :model-value="filteredFiles.length > 0 && filteredFiles.every((f) => selected.has(f.relativePath))"
          :indeterminate="selectedCount > 0 && selectedCount < filteredFiles.length"
          @change="toggleAll(!!$event)"
        >
          全选当前列表
        </el-checkbox>
        <el-input v-model="listFilter" placeholder="在结果中筛选…" clearable style="width:200px" />
        <el-button type="danger" plain :disabled="!selectedCount" @click="deleteSelected">
          删除选中
        </el-button>
        <el-button plain :disabled="!selectedCount" @click="openMoveDialog">移动到…</el-button>
        <el-button plain :disabled="!selectedCount" @click="openRenameDialog">批量重命名</el-button>
        <el-dropdown trigger="click" @command="(cmd: string) => exportResults(cmd.endsWith('-sel') ? 'selected' : 'all', cmd.split('-')[0] as 'txt' | 'csv' | 'json')">
          <el-button plain>
            导出列表
            <el-icon class="el-icon--right"><ArrowDown /></el-icon>
          </el-button>
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item command="txt">全部 → TXT（路径）</el-dropdown-item>
              <el-dropdown-item command="csv">全部 → CSV</el-dropdown-item>
              <el-dropdown-item command="json">全部 → JSON</el-dropdown-item>
              <el-dropdown-item divided command="txt-sel">选中 → TXT</el-dropdown-item>
              <el-dropdown-item command="csv-sel">选中 → CSV</el-dropdown-item>
              <el-dropdown-item command="json-sel">选中 → JSON</el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
      </div>
    </div>

    <div class="find-list">
      <div v-for="f in filteredFiles" :key="f.relativePath" class="find-file">
        <el-checkbox
          :model-value="selected.has(f.relativePath)"
          @change="toggleFile(f.relativePath, !!$event)"
        />
        <span class="path-cell">{{ f.relativePath }}</span>
        <span class="meta-cell">{{ formatSize(f.size) }}</span>
        <span class="meta-cell muted">{{ formatMtime(f.mtime) }}</span>
      </div>
      <div v-if="!files.length && !loading" class="empty-state">
        <el-icon><Search /></el-icon>
        <p>选择文件夹并设置匹配规则后搜索</p>
      </div>
      <div v-else-if="files.length && !filteredFiles.length" class="empty-state">
        <p>当前筛选无结果</p>
      </div>
    </div>

    <el-dialog v-model="moveDialogVisible" title="移动到文件夹" width="520px">
      <div class="dialog-field">
        <label>目标文件夹</label>
        <div class="dialog-row">
          <el-input v-model="moveTargetDir" readonly placeholder="点击右侧按钮选择" />
          <el-button @click="pickMoveTarget">选择</el-button>
        </div>
      </div>
      <el-checkbox v-model="preserveStructure">保留子目录结构（否则仅移动文件名到目标根目录）</el-checkbox>
      <p class="mode-tip">将移动 {{ selectedCount }} 个文件到目标位置。</p>
      <template #footer>
        <el-button @click="moveDialogVisible = false">取消</el-button>
        <el-button type="primary" :loading="loading" @click="confirmMove">确认移动</el-button>
      </template>
    </el-dialog>

    <el-dialog v-model="renameDialogVisible" title="批量重命名选中文件" width="720px" class="rename-dialog">
      <el-tabs v-model="renameActiveTab" class="rename-rule-tabs">
        <el-tab-pane label="基础" name="basic">
          <div class="rename-rules">
            <div class="rule-row">
              <span>前缀</span>
              <el-input v-model="renameRules.prefix" placeholder="添加到文件名前" />
            </div>
            <div class="rule-row">
              <span>后缀</span>
              <el-input v-model="renameRules.suffix" placeholder="添加到扩展名前" />
            </div>
            <div class="rule-row pair">
              <span>查找</span>
              <el-input v-model="renameRules.replaceFrom" placeholder="被替换文本" />
              <span class="arrow">→</span>
              <el-input v-model="renameRules.replaceTo" placeholder="新文本" />
            </div>
            <div class="rule-block">
              <label>删除内容（支持正则，逗号/换行分隔）</label>
              <el-input v-model="removeText" type="textarea" :rows="2" placeholder="如 _copy 或 \\d+$" />
            </div>
          </div>
        </el-tab-pane>
        <el-tab-pane label="按位置删除" name="deleteAt">
          <el-switch v-model="renameRules.deleteAt!.enabled" active-text="启用按位置删除" />
          <p class="rule-hint">仅作用于主文件名（不含扩展名）。先保留前 N 个字符，再删除其后 M 个。</p>
          <div class="rule-grid">
            <div class="rule-block">
              <label>保留前几个字符</label>
              <el-input-number v-model="renameRules.deleteAt!.start" :min="0" :max="999" controls-position="right" />
            </div>
            <div class="rule-block">
              <label>删除几个字符</label>
              <el-input-number v-model="renameRules.deleteAt!.count" :min="0" :max="999" controls-position="right" />
            </div>
          </div>
          <p class="rule-hint">示例：<code>123456</code> → <code>{{ deleteAtPreview }}</code></p>
        </el-tab-pane>
        <el-tab-pane label="自然数序号" name="sequence">
          <el-switch v-model="renameRules.sequence!.enabled" active-text="启用序号" />
          <div class="rule-block">
            <label>位置</label>
            <el-radio-group v-model="renameRules.sequence!.position">
              <el-radio value="prefix">前缀</el-radio>
              <el-radio value="suffix">后缀</el-radio>
              <el-radio value="insert">中间插入</el-radio>
            </el-radio-group>
          </div>
          <div v-if="renameRules.sequence!.position === 'insert'" class="rule-block">
            <label>插入位置（第几个字符，0 起）</label>
            <el-input-number v-model="renameRules.sequence!.insertIndex" :min="0" controls-position="right" />
          </div>
          <div class="rule-grid">
            <div class="rule-block">
              <label>起始数</label>
              <el-input-number v-model="renameRules.sequence!.start" controls-position="right" />
            </div>
            <div class="rule-block">
              <label>步长</label>
              <el-input-number v-model="renameRules.sequence!.step" :min="1" controls-position="right" />
            </div>
            <div class="rule-block">
              <label>位数补零</label>
              <el-input-number v-model="renameRules.sequence!.padWidth" :min="0" :max="8" controls-position="right" />
            </div>
          </div>
          <div class="rule-block">
            <label>分隔符</label>
            <el-input v-model="renameRules.sequence!.separator" placeholder="如 _ 或 -" />
          </div>
          <p class="rule-hint">按选中文件列表顺序依次编号</p>
        </el-tab-pane>
        <el-tab-pane label="指定位置插入" name="insert">
          <el-switch v-model="renameRules.insert!.enabled" active-text="启用插入" />
          <div class="rule-block">
            <label>字符位置（0 起）</label>
            <el-input-number v-model="renameRules.insert!.index" :min="0" controls-position="right" />
          </div>
          <el-switch v-model="renameRules.insert!.useSequence" active-text="插入内容为自然数（用序号设置）" />
          <div v-if="!renameRules.insert!.useSequence" class="rule-block">
            <label>插入内容</label>
            <el-input v-model="renameRules.insert!.content" />
          </div>
        </el-tab-pane>
      </el-tabs>
      <div class="rename-actions">
        <el-button :loading="renameLoading" @click="previewRenamePlan">预览 ({{ selectedCount }} 个文件)</el-button>
      </div>
      <div v-if="renamePlan.length" class="rename-preview">
        <div v-for="item in renamePlan" :key="item.relativePath" class="rename-item">
          <span class="path">{{ item.relativePath }}</span>
          <span class="old">{{ item.oldName }}</span>
          <el-icon><Right /></el-icon>
          <span class="new" :class="item.status">{{ item.newName }}</span>
          <span v-if="item.reason" class="reason">{{ item.reason }}</span>
        </div>
      </div>
      <template #footer>
        <el-button @click="renameDialogVisible = false">取消</el-button>
        <el-button type="primary" :loading="renameLoading" :disabled="!renameReady.length" @click="applyRename">
          执行重命名 ({{ renameReady.length }})
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped lang="scss">
.find-page {
  padding: 20px 24px;
  height: calc(100vh - 56px);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.find-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 14px;

  h2 {
    margin: 0 0 4px;
    font-size: 20px;
  }

  .hint {
    margin: 0;
    font-size: 13px;
    color: var(--text-muted);
  }
}

.find-toolbar {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  margin-bottom: 12px;
}

.search-card {
  padding: 14px 16px;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--surface);
  margin-bottom: 12px;
}

.search-row {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 10px;

  label {
    width: 72px;
    flex-shrink: 0;
    font-size: 13px;
    color: var(--text-muted);
  }

  .el-input {
    flex: 1;
    max-width: 520px;
  }
}

.search-options {
  display: flex;
  gap: 16px;
  margin-bottom: 6px;
}

.size-filter-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px 10px;
  margin-bottom: 6px;

  .size-label {
    font-size: 12px;
    color: var(--text-muted);
  }

  .size-input {
    width: 120px;
  }

  .size-unit {
    width: 80px;
  }

  .inline {
    margin: 0;
  }
}

.mode-tip {
  margin: 0;
  font-size: 12px;
  color: var(--text-muted);
  line-height: 1.6;
}

.result-bar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px 12px;
  margin-bottom: 10px;

  .result-actions {
    margin-left: auto;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
}

.find-list {
  flex: 1;
  overflow-y: auto;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--surface);
}

.find-file {
  display: grid;
  grid-template-columns: auto 1fr 88px 160px;
  align-items: center;
  gap: 10px;
  padding: 8px 14px;
  border-bottom: 1px solid var(--border);
  font-size: 12px;

  &:last-child {
    border-bottom: none;
  }
}

.path-cell {
  word-break: break-all;
}

.meta-cell {
  text-align: right;
  white-space: nowrap;

  &.muted {
    color: var(--text-muted);
    font-size: 11px;
  }
}

.dialog-field {
  margin-bottom: 12px;

  label {
    display: block;
    margin-bottom: 6px;
    font-size: 13px;
  }
}

.dialog-row {
  display: flex;
  gap: 8px;
}

.rename-rules {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin-bottom: 12px;
}

.rule-row {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;

  span:first-child {
    width: 48px;
    flex-shrink: 0;
    color: var(--text-muted);
  }

  &.pair {
    flex-wrap: wrap;

    .arrow {
      width: auto;
      color: var(--text-muted);
    }
  }

  .el-input {
    flex: 1;
  }
}

.rule-block {
  margin-bottom: 10px;

  label {
    display: block;
    margin-bottom: 6px;
    font-size: 12px;
    color: var(--text-muted);
  }
}

.rule-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
  gap: 10px;
  margin-bottom: 8px;
}

.rule-hint {
  margin: 8px 0;
  font-size: 12px;
  color: var(--text-muted);
  line-height: 1.5;
}

.rename-rule-tabs {
  margin-bottom: 8px;
}

.rename-actions {
  margin-bottom: 10px;
}

.rename-preview {
  max-height: 240px;
  overflow-y: auto;
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 8px;
  background: var(--surface-2);
}

.rename-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 0;
  font-size: 12px;
  flex-wrap: wrap;

  .path {
    width: 100%;
    color: var(--text-muted);
    font-size: 11px;
  }

  .old {
    color: var(--text-muted);
  }

  .new {
    font-weight: 600;

    &.collision,
    &.invalid {
      color: #ef4444;
    }
  }

  .reason {
    color: #ef4444;
    font-size: 11px;
  }
}
</style>
