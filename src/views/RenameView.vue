<script setup lang="ts">
import { computed, reactive, ref } from 'vue';
import { ElMessage, ElMessageBox } from 'element-plus';
import { executeRename, openFolder, previewRename, sanitizeNames } from '@/api';
import ClearCacheButton from '@/components/ClearCacheButton.vue';
import FavoritePathInput from '@/components/FavoritePathInput.vue';
import type { RenamePlanItem, RenameRules } from '@/types';

const rootPath = ref('');
const recursive = ref(true);
const scope = ref<'files' | 'directories' | 'both'>('files');
const loading = ref(false);
const plan = ref<RenamePlanItem[]>([]);
const stats = ref<Record<string, number>>({});
const selected = ref<Set<string>>(new Set());
const activeTab = ref('basic');

const rules = reactive<RenameRules>({
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

const removeText = ref('');
const filteredPlan = computed(() => plan.value.filter((p) => p.status !== 'unchanged'));
const readyPlan = computed(() => plan.value.filter((p) => p.status === 'ready'));
const selectedReady = computed(() => readyPlan.value.filter((p) => selected.value.has(p.relativePath)));

const deleteAtPreview = computed(() => {
  const keep = rules.deleteAt?.start ?? 0;
  const count = rules.deleteAt?.count ?? 0;
  const sample = '123456';
  if (!count || count <= 0) return sample;
  if (keep >= sample.length) return sample;
  return sample.slice(0, keep) + sample.slice(keep + count);
});

async function runPreview() {
  if (!rootPath.value) return ElMessage.warning('请选择文件夹');
  loading.value = true;
  selected.value = new Set();
  try {
    rules.removePatterns = removeText.value.split(/[/、，,\n]+/).map((s) => s.trim()).filter(Boolean);
    const res = await previewRename({ rootPath: rootPath.value, rules: { ...rules }, recursive: recursive.value, scope: scope.value });
    plan.value = res.plan;
    stats.value = res.stats;
    ElMessage.success(`预览完成：${res.stats.ready} 项将重命名`);
  } catch (err: any) {
    ElMessage.error(err.message);
  } finally {
    loading.value = false;
  }
}

async function runExecute(onlySelected = false) {
  const items = onlySelected ? selectedReady.value : readyPlan.value;
  if (!items.length) return ElMessage.warning('没有可执行项');
  try {
    await ElMessageBox.confirm(`确认重命名 ${items.length} 项？`, '批量重命名', { type: 'warning' });
    loading.value = true;
    const res = await executeRename(rootPath.value, items);
    ElMessage.success(`成功 ${res.renamed.length} 项${res.errors.length ? `，失败 ${res.errors.length}` : ''}`);
    await runPreview();
  } catch (err: any) {
    if (err !== 'cancel') ElMessage.error(err.message || '失败');
  } finally {
    loading.value = false;
  }
}

async function runSanitize() {
  if (!rootPath.value) return ElMessage.warning('请选择文件夹');
  try {
    await ElMessageBox.confirm('将冒号、斜杠等非法字符替换为下划线，是否继续？', '清理文件名');
    loading.value = true;
    const res = await sanitizeNames(rootPath.value, scope.value);
    ElMessage.success(`已处理 ${res.renamed.length} 项`);
    await runPreview();
  } catch (err: any) {
    if (err !== 'cancel') ElMessage.error(err.message);
  } finally {
    loading.value = false;
  }
}

function toggleAll(checked: boolean) {
  selected.value = checked ? new Set(readyPlan.value.map((p) => p.relativePath)) : new Set();
}

function toggleSelect(path: string, v: boolean) {
  const s = new Set(selected.value);
  if (v) s.add(path);
  else s.delete(path);
  selected.value = s;
}

function statusType(s: string) {
  if (s === 'ready') return 'success';
  if (s === 'collision') return 'danger';
  if (s === 'invalid') return 'danger';
  return 'info';
}

function handleClearRename() {
  rootPath.value = '';
  recursive.value = true;
  scope.value = 'files';
  plan.value = [];
  stats.value = {};
  selected.value = new Set();
  activeTab.value = 'basic';
  rules.prefix = '';
  rules.suffix = '';
  rules.replaceFrom = '';
  rules.replaceTo = '';
  rules.removePatterns = [];
  if (rules.sequence) rules.sequence.enabled = false;
  if (rules.insert) rules.insert.enabled = false;
  if (rules.deleteAt) rules.deleteAt.enabled = false;
  removeText.value = '';
}
</script>

<template>
  <div class="page rename-page">
    <div class="rename-layout">
      <aside class="rename-rules">
        <div class="rename-panel">
          <div class="rename-panel-head">
            <h3 class="rename-panel-title">目标文件夹</h3>
            <ClearCacheButton module="rename" @cleared="handleClearRename" />
          </div>
          <div class="rename-path-row">
            <FavoritePathInput v-model="rootPath" placeholder="文件夹路径" class="rename-path-input" />
            <div class="rename-path-actions">
              <el-button v-if="rootPath" title="在 Finder 中打开" @click="openFolder(rootPath)">
                <el-icon><Position /></el-icon>
              </el-button>
            </div>
          </div>
          <div class="rename-opts-row">
            <el-checkbox v-model="recursive">包含子文件夹</el-checkbox>
            <el-select v-model="scope" size="small" class="rename-scope-select">
              <el-option label="仅文件" value="files" />
              <el-option label="仅文件夹" value="directories" />
              <el-option label="文件+文件夹" value="both" />
            </el-select>
          </div>
          <div class="rename-primary-actions">
            <el-button type="primary" :loading="loading" @click="runPreview">预览重命名</el-button>
            <el-button @click="runSanitize">一键清理非法字符</el-button>
          </div>
        </div>

        <el-tabs v-model="activeTab" class="rename-rule-tabs">
          <el-tab-pane label="基础" name="basic">
            <div class="rule-field">
              <label>前缀</label>
              <el-input v-model="rules.prefix" placeholder="加到文件名前" />
            </div>
            <div class="rule-field">
              <label>后缀</label>
              <el-input v-model="rules.suffix" placeholder="加到扩展名前" />
            </div>
            <div class="rule-field">
              <label>查找替换</label>
              <div class="pair-row">
                <el-input v-model="rules.replaceFrom" placeholder="查找" />
                <span class="pair-arrow">→</span>
                <el-input v-model="rules.replaceTo" placeholder="替换为" />
              </div>
            </div>
            <div class="rule-field">
              <label>删除内容</label>
              <el-input v-model="removeText" type="textarea" :rows="2" placeholder="多个用逗号或换行分隔，支持正则" />
            </div>
          </el-tab-pane>

          <el-tab-pane label="按位置删除" name="deleteAt">
            <el-switch v-model="rules.deleteAt!.enabled" active-text="启用按位置删除" />
            <p class="rule-hint">仅作用于主文件名（不含扩展名）。先保留前 N 个字符，再删除其后 M 个字符。</p>
            <div class="rule-grid rule-grid-2">
              <div class="rule-field">
                <label>保留前几个字符</label>
                <el-input-number v-model="rules.deleteAt!.start" :min="0" :max="999" controls-position="right" class="rule-number" />
              </div>
              <div class="rule-field">
                <label>删除几个字符</label>
                <el-input-number v-model="rules.deleteAt!.count" :min="0" :max="999" controls-position="right" class="rule-number" />
              </div>
            </div>
            <div class="rule-example">
              示例：<code>123456</code> 保留前 <strong>{{ rules.deleteAt!.start }}</strong> 个，删 <strong>{{ rules.deleteAt!.count }}</strong> 个 → <code>{{ deleteAtPreview }}</code>
            </div>
          </el-tab-pane>

          <el-tab-pane label="自然数序号" name="sequence">
            <el-switch v-model="rules.sequence!.enabled" active-text="启用序号" />
            <div class="rule-field">
              <label>位置</label>
              <el-radio-group v-model="rules.sequence!.position" class="rule-radio-group">
                <el-radio value="prefix">前缀</el-radio>
                <el-radio value="suffix">后缀</el-radio>
                <el-radio value="insert">中间插入</el-radio>
              </el-radio-group>
            </div>
            <div v-if="rules.sequence!.position === 'insert'" class="rule-field">
              <label>插入位置（第几个字符，0 起）</label>
              <el-input-number v-model="rules.sequence!.insertIndex" :min="0" controls-position="right" class="rule-number" />
            </div>
            <div class="rule-grid">
              <div class="rule-field">
                <label>起始数</label>
                <el-input-number v-model="rules.sequence!.start" controls-position="right" class="rule-number" />
              </div>
              <div class="rule-field">
                <label>步长</label>
                <el-input-number v-model="rules.sequence!.step" :min="1" controls-position="right" class="rule-number" />
              </div>
              <div class="rule-field">
                <label>位数补零</label>
                <el-input-number v-model="rules.sequence!.padWidth" :min="0" :max="8" controls-position="right" class="rule-number" />
              </div>
            </div>
            <div class="rule-field">
              <label>分隔符</label>
              <el-input v-model="rules.sequence!.separator" placeholder="如 _ 或 -" />
            </div>
            <p class="rule-hint">按排序顺序依次编号，如 01、02… 或 001、002…</p>
          </el-tab-pane>

          <el-tab-pane label="指定位置插入" name="insert">
            <el-switch v-model="rules.insert!.enabled" active-text="启用插入" />
            <div class="rule-field">
              <label>字符位置（0 起）</label>
              <el-input-number v-model="rules.insert!.index" :min="0" controls-position="right" class="rule-number" />
            </div>
            <el-switch v-model="rules.insert!.useSequence" active-text="插入内容为自然数（用序号设置）" />
            <div v-if="!rules.insert!.useSequence" class="rule-field">
              <label>插入内容</label>
              <el-input v-model="rules.insert!.content" />
            </div>
            <p class="rule-hint">在文件主名称（不含扩展名）的指定位置插入文字或序号</p>
          </el-tab-pane>
        </el-tabs>
      </aside>

      <main class="rename-preview">
        <div v-if="plan.length" class="rename-stats">
          <el-tag>总计 {{ stats.total }}</el-tag>
          <el-tag type="success">可执行 {{ stats.ready }}</el-tag>
          <el-tag type="info">未变 {{ stats.unchanged }}</el-tag>
          <el-tag type="danger">冲突 {{ stats.collision }}</el-tag>
          <el-tag>已选 {{ selectedReady.length }}</el-tag>
        </div>
        <div class="rename-table-wrap">
          <el-table v-if="filteredPlan.length" :data="filteredPlan" height="100%" size="small" stripe v-loading="loading">
            <el-table-column width="44">
              <template #header>
                <el-checkbox :model-value="selected.size === readyPlan.length && readyPlan.length > 0" @change="toggleAll(!!$event)" />
              </template>
              <template #default="{ row }">
                <el-checkbox
                  v-if="row.status === 'ready'"
                  :model-value="selected.has(row.relativePath)"
                  @change="(v: boolean) => toggleSelect(row.relativePath, v)"
                />
              </template>
            </el-table-column>
            <el-table-column label="状态" width="90">
              <template #default="{ row }"><el-tag size="small" :type="statusType(row.status)">{{ row.status }}</el-tag></template>
            </el-table-column>
            <el-table-column label="原路径" min-width="200" show-overflow-tooltip prop="relativePath" />
            <el-table-column label="原文件名" width="160" prop="oldName" />
            <el-table-column label="新文件名" width="160">
              <template #default="{ row }"><span class="new-name">{{ row.newName }}</span></template>
            </el-table-column>
            <el-table-column label="说明" min-width="120" prop="reason" />
          </el-table>
          <div v-else class="empty-state">
            <el-icon><EditPen /></el-icon>
            <p>配置规则后点击「预览重命名」</p>
          </div>
        </div>
        <footer class="rename-action-bar">
          <el-button type="primary" :disabled="!readyPlan.length" :loading="loading" @click="runExecute(false)">
            执行全部 ({{ readyPlan.length }})
          </el-button>
          <el-button :disabled="!selectedReady.length" :loading="loading" @click="runExecute(true)">
            执行选中 ({{ selectedReady.length }})
          </el-button>
        </footer>
      </main>
    </div>
  </div>
</template>

<style scoped lang="scss">
.rename-page {
  height: calc(100vh - 56px);
  min-height: 0;
}

.rename-layout {
  display: flex;
  height: 100%;
  min-height: 0;
}

.rename-rules {
  width: 380px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow-y: auto;
  background: var(--surface);
  border-right: 1px solid var(--border);
  padding: 16px;
}

.rename-panel {
  padding: 14px;
  background: var(--surface-2);
  border: 1px solid var(--border);
  border-radius: 10px;
}

.rename-panel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 12px;
}

.rename-panel-title {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
  color: var(--text-muted);
  letter-spacing: 0.04em;
}

.rename-path-row {
  display: flex;
  align-items: stretch;
  gap: 8px;
}

.rename-path-input {
  flex: 1;
  min-width: 0;
}

.rename-path-actions {
  display: flex;
  flex-shrink: 0;
  gap: 6px;

  .el-button {
    margin: 0;
    width: 36px;
    height: 32px;
    padding: 0;
  }
}

.rename-opts-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-top: 12px;
  flex-wrap: wrap;
}

.rename-scope-select {
  width: 132px;
  flex-shrink: 0;
}

.rename-primary-actions {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-top: 14px;

  .el-button {
    margin: 0;
    width: 100%;
  }
}

.rename-rule-tabs {
  margin-top: 14px;
  flex: 1;
  min-height: 0;

  :deep(.el-tabs__header) {
    margin-bottom: 12px;
  }

  :deep(.el-tabs__item) {
    font-size: 13px;
    padding: 0 12px;
  }

  :deep(.el-tabs__content) {
    overflow: visible;
  }

  :deep(.el-switch) {
    margin-bottom: 10px;
  }
}

.rule-field {
  margin-bottom: 12px;

  label {
    display: block;
    font-size: 12px;
    color: var(--text-muted);
    margin-bottom: 6px;
  }
}

.rule-number {
  width: 100%;
}

.pair-row {
  display: grid;
  grid-template-columns: 1fr auto 1fr;
  align-items: center;
  gap: 8px;
}

.pair-arrow {
  color: var(--text-muted);
  font-size: 14px;
  line-height: 1;
}

.rule-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 10px;
}

.rule-grid-2 {
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.rule-radio-group {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 12px;
}

.rule-hint {
  margin: 8px 0 0;
  font-size: 12px;
  color: var(--text-muted);
  line-height: 1.5;
}

.rule-example {
  margin-top: 4px;
  padding: 10px 12px;
  font-size: 12px;
  line-height: 1.6;
  color: var(--text-muted);
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: 8px;

  code {
    padding: 1px 6px;
    border-radius: 4px;
    background: var(--surface-2);
    color: var(--text);
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }

  strong {
    color: var(--text);
    font-weight: 600;
  }
}

.rename-preview {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  background: var(--bg);
}

.rename-stats {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  padding: 12px 16px;
  background: var(--surface);
  border-bottom: 1px solid var(--border);
  flex-shrink: 0;
}

.rename-table-wrap {
  flex: 1;
  min-height: 0;
  margin: 12px 16px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 10px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.rename-table-wrap .el-table {
  flex: 1;
}

.new-name {
  color: #86efac;
  font-weight: 600;
}

.rename-action-bar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
  padding: 12px 16px;
  background: var(--surface);
  border-top: 1px solid var(--border);
  flex-shrink: 0;

  .el-button {
    margin: 0;
  }
}
</style>
