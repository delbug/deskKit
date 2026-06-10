<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue';
import { ElMessage } from 'element-plus';
import type { ElTree } from 'element-plus';
import ClearCacheButton from '@/components/ClearCacheButton.vue';
import FavoritePathInput from '@/components/FavoritePathInput.vue';
import {
  convertToConfluence,
  getHealth,
  listConfluenceFiles,
  openFolder,
  pickFolder,
  previewConfluenceFile,
  type ConfluenceOutputFormat,
} from '@/api';
import { YUQUE_DISCLAIMER_SHORT } from '@/constants/yuque-disclaimer';

type MdFileItem = { absolutePath: string; relativePath: string; fileName: string };

interface FileTreeNode {
  id: string;
  label: string;
  isFile?: boolean;
  absolutePath?: string;
  children?: FileTreeNode[];
}

const sourceDir = ref('');
const outputDir = ref('');
const outputFormat = ref<ConfluenceOutputFormat>('html');
const sameDir = ref(true);
const recursive = ref(true);
const overwrite = ref(false);
const loading = ref(false);
const serverReady = ref(true);
const serverHint = ref('');
const fileList = ref<MdFileItem[]>([]);
const selectedPaths = ref<string[]>([]);
const fileTreeRef = ref<InstanceType<typeof ElTree>>();
const previewFile = ref('');
const previewHtml = ref('');
const previewTitle = ref('');
const lastResult = ref<{
  convertedCount: number;
  skippedCount: number;
  failedCount: number;
  total: number;
  outputDir: string;
  failed?: { relativePath: string; message: string }[];
  skipped?: { relativePath: string; outputPath: string; reason?: string }[];
} | null>(null);

const previewSrcdoc = computed(() => previewHtml.value || '<p style="color:#888;padding:24px">选择文件后点击「预览转换」</p>');

const formatExt = computed(() => {
  if (outputFormat.value === 'html') return '.html';
  if (outputFormat.value === 'md') return '.md';
  if (outputFormat.value === 'pdf') return '.pdf';
  return '.docx';
});

const formatLabel = computed(() => {
  if (outputFormat.value === 'html') return 'Confluence 网页';
  if (outputFormat.value === 'md') return 'Markdown';
  if (outputFormat.value === 'pdf') return 'PDF';
  return 'Word';
});

const convertButtonText = computed(() => {
  const n = selectedPaths.value.length;
  if (!n) return `导出为 ${formatLabel.value}`;
  return `导出已选 ${n} 个为 ${formatLabel.value}`;
});

const selectedCount = computed(() => selectedPaths.value.length);

const fileTreeData = computed(() => buildFileTree(fileList.value));

const resultTip = computed(() => {
  if (outputFormat.value === 'html') {
    return '用浏览器打开 .html → Cmd+A 全选 → 复制 → 粘贴到 Confluence（图片会一起带上）';
  }
  if (outputFormat.value === 'docx') {
    return 'Word 批量复制时 Confluence 常丢失图片；粘贴到 Confluence 请优先用「Confluence 网页」格式';
  }
  if (outputFormat.value === 'md') {
    return '已导出规范化 Markdown，语雀绘图注释会转为 mermaid 代码块';
  }
  return 'PDF 适合归档、打印或分享；流程图会渲染为图片';
});

const formatHint = computed(() => {
  if (outputFormat.value === 'html') {
    return '推荐粘贴到 Confluence：图片内嵌在网页里，浏览器全选复制后图文一起粘贴。';
  }
  if (outputFormat.value === 'docx') {
    return 'Word 适合本地编辑；粘贴到 Confluence 时图片可能不会随正文一起带上，请改用 Confluence 网页格式。';
  }
  if (outputFormat.value === 'md') {
    return 'Markdown 会保留源码结构，并规范化语雀特有的绘图注释。';
  }
  return 'PDF 由排版预览生成，转换较慢，需联网渲染流程图。';
});

function buildFileTree(files: MdFileItem[]): FileTreeNode[] {
  const roots: FileTreeNode[] = [];

  for (const file of files) {
    const parts = file.relativePath.split('/').filter(Boolean);
    let siblings = roots;

    for (let i = 0; i < parts.length; i += 1) {
      const part = parts[i];
      const isFile = i === parts.length - 1;
      const folderId = parts.slice(0, i + 1).join('/');
      const id = isFile ? file.absolutePath : folderId;

      let node = siblings.find((item) => item.id === id);
      if (!node) {
        node = {
          id,
          label: part,
          isFile,
          absolutePath: isFile ? file.absolutePath : undefined,
          children: isFile ? undefined : [],
        };
        siblings.push(node);
      }
      if (!isFile && node.children) {
        siblings = node.children;
      }
    }
  }

  const sortNodes = (nodes: FileTreeNode[]) => {
    nodes.sort((a, b) => {
      if (!!a.isFile !== !!b.isFile) return a.isFile ? 1 : -1;
      return a.label.localeCompare(b.label, 'zh-CN');
    });
    for (const node of nodes) {
      if (node.children?.length) sortNodes(node.children);
    }
  };
  sortNodes(roots);
  return roots;
}

function persistSelectedFiles() {
  localStorage.setItem('confluence-selected-files', JSON.stringify(selectedPaths.value));
}

function loadSelectedFiles(validPaths: string[]) {
  try {
    const saved = JSON.parse(localStorage.getItem('confluence-selected-files') || '[]');
    if (!Array.isArray(saved)) return validPaths;
    const valid = saved.filter((p: string) => validPaths.includes(p));
    return valid.length ? valid : validPaths;
  } catch {
    return validPaths;
  }
}

function applyTreeCheckedKeys(keys: string[]) {
  nextTick(() => {
    fileTreeRef.value?.setCheckedKeys(keys, false);
  });
}

function syncSelectionAfterScan(files: MdFileItem[]) {
  const allPaths = files.map((f) => f.absolutePath);
  selectedPaths.value = loadSelectedFiles(allPaths);
  applyTreeCheckedKeys(selectedPaths.value);
  persistSelectedFiles();
}

function onTreeCheck() {
  const leafNodes = (fileTreeRef.value?.getCheckedNodes(true) || []) as FileTreeNode[];
  selectedPaths.value = leafNodes
    .filter((node) => node.isFile && node.absolutePath)
    .map((node) => node.absolutePath!);
  persistSelectedFiles();
}

function selectAllFiles() {
  selectedPaths.value = fileList.value.map((f) => f.absolutePath);
  applyTreeCheckedKeys(selectedPaths.value);
  persistSelectedFiles();
}

function selectNoneFiles() {
  selectedPaths.value = [];
  applyTreeCheckedKeys([]);
  persistSelectedFiles();
}

function invertSelection() {
  const all = new Set(fileList.value.map((f) => f.absolutePath));
  const next = [...all].filter((p) => !selectedPaths.value.includes(p));
  selectedPaths.value = next;
  applyTreeCheckedKeys(next);
  persistSelectedFiles();
}

function loadSettings() {
  const savedSource = localStorage.getItem('confluence-source-dir');
  if (savedSource) sourceDir.value = savedSource;

  const savedOutput = localStorage.getItem('confluence-output-dir');
  if (savedOutput) outputDir.value = savedOutput;

  const savedSame = localStorage.getItem('confluence-same-dir');
  if (savedSame === '0') sameDir.value = false;
  else if (savedSame === '1') sameDir.value = true;

  const savedRecursive = localStorage.getItem('confluence-recursive');
  if (savedRecursive === '0') recursive.value = false;
  else if (savedRecursive === '1') recursive.value = true;

  const savedOverwrite = localStorage.getItem('confluence-overwrite');
  if (savedOverwrite === '1') overwrite.value = true;

  const savedFormat = localStorage.getItem('confluence-output-format');
  if (savedFormat === 'html' || savedFormat === 'md' || savedFormat === 'pdf' || savedFormat === 'docx') {
    outputFormat.value = savedFormat;
  }
}

function persistSettings() {
  if (sourceDir.value.trim()) {
    localStorage.setItem('confluence-source-dir', sourceDir.value.trim());
  }
  if (outputDir.value.trim()) {
    localStorage.setItem('confluence-output-dir', outputDir.value.trim());
  }
  localStorage.setItem('confluence-same-dir', sameDir.value ? '1' : '0');
  localStorage.setItem('confluence-recursive', recursive.value ? '1' : '0');
  localStorage.setItem('confluence-overwrite', overwrite.value ? '1' : '0');
  localStorage.setItem('confluence-output-format', outputFormat.value);
  if (previewFile.value) {
    localStorage.setItem('confluence-preview-file', previewFile.value);
  }
}

watch([sourceDir, outputDir, sameDir, recursive, overwrite, outputFormat], persistSettings);

watch(recursive, () => {
  if (sourceDir.value.trim()) scanFiles();
});

let scanTimer: ReturnType<typeof setTimeout> | null = null;
watch(sourceDir, (dir) => {
  if (scanTimer) clearTimeout(scanTimer);
  const trimmed = dir.trim();
  if (!trimmed) {
    fileList.value = [];
    selectedPaths.value = [];
    previewFile.value = '';
    return;
  }
  scanTimer = setTimeout(() => {
    scanFiles();
  }, 400);
});

function syncPreviewFileAfterScan(files: typeof fileList.value) {
  const first = files[0];
  if (!first) {
    previewFile.value = '';
    return;
  }
  const stillValid = files.some((f) => f.absolutePath === previewFile.value);
  if (!stillValid) {
    previewFile.value = first.absolutePath;
  }
}

async function checkServerReady() {
  try {
    const data = await getHealth();
    const features: string[] = data.features || [];
    if (!features.includes('confluence')) {
      serverReady.value = false;
      serverHint.value = '当前版本缺少 Confluence 功能，请更新 DeskKit 到最新版本。';
    } else {
      serverReady.value = true;
      serverHint.value = '';
    }
  } catch {
    serverReady.value = false;
    serverHint.value = 'DeskKit 后端未就绪，请重启应用。';
  }
}

onMounted(async () => {
  loadSettings();
  await checkServerReady();
  if (sourceDir.value.trim()) {
    await scanFiles();
  }
});

async function pickSourceDir() {
  const res = await pickFolder();
  if (!res.cancelled) {
    if (scanTimer) clearTimeout(scanTimer);
    sourceDir.value = res.path;
    persistSettings();
    await scanFiles();
  }
}

async function pickOutputDir() {
  const res = await pickFolder();
  if (!res.cancelled) {
    outputDir.value = res.path;
    persistSettings();
  }
}

async function scanFiles() {
  if (!sourceDir.value.trim()) return;
  loading.value = true;
  try {
    const res = await listConfluenceFiles(sourceDir.value.trim(), recursive.value);
    fileList.value = res.files;
    syncPreviewFileAfterScan(res.files);
    syncSelectionAfterScan(res.files);
    persistSettings();
    if (res.files.length === 0) {
      ElMessage.warning(
        recursive.value
          ? '当前目录下未找到 .md 文件，请确认是否选对了语雀导出文件夹'
          : '当前目录下未找到 .md 文件，语雀导出通常在子文件夹中，请勾选「包含子文件夹」',
      );
    }
  } catch (err: any) {
    ElMessage.error(err.message);
    fileList.value = [];
    selectedPaths.value = [];
    previewFile.value = '';
  } finally {
    loading.value = false;
  }
}

async function handlePreview() {
  if (!previewFile.value) return ElMessage.warning('请选择要预览的 Markdown 文件');
  loading.value = true;
  try {
    const res = await previewConfluenceFile(previewFile.value);
    previewHtml.value = res.html;
    previewTitle.value = res.title;
    persistSettings();
    if (res.imagesFailed?.length) {
      ElMessage.warning(`预览完成，${res.imagesEmbedded || 0} 张图片已加载，${res.imagesFailed.length} 张远程图片下载失败`);
    } else if (res.imagesEmbedded) {
      ElMessage.success(`预览：${res.title}（已加载 ${res.imagesEmbedded} 张图片）`);
    } else {
      ElMessage.success(`预览：${res.title}`);
    }
  } catch (err: any) {
    ElMessage.error(err.message);
  } finally {
    loading.value = false;
  }
}

async function handleConvert() {
  if (!sourceDir.value.trim()) return ElMessage.warning('请选择 Markdown 源目录');
  if (!sameDir.value && !outputDir.value.trim()) {
    return ElMessage.warning('请选择输出目录，或勾选「输出到源目录」');
  }
  if (!selectedPaths.value.length) {
    return ElMessage.warning('请至少勾选一个要导出的文件');
  }

  loading.value = true;
  lastResult.value = null;
  try {
    const res = await convertToConfluence({
      sourceDir: sourceDir.value.trim(),
      outputDir: outputDir.value.trim() || undefined,
      sameDir: sameDir.value,
      recursive: recursive.value,
      overwrite: overwrite.value,
      format: outputFormat.value,
      files: selectedPaths.value,
    });
    lastResult.value = {
      convertedCount: res.convertedCount,
      skippedCount: res.skippedCount,
      failedCount: res.failedCount,
      total: res.total,
      outputDir: res.outputDir,
      failed: res.failed,
      skipped: res.skipped,
    };
    if (res.failedCount) {
      ElMessage.warning(`转换完成：成功 ${res.convertedCount}，跳过 ${res.skippedCount}，失败 ${res.failedCount}（详见下方失败列表）`);
    } else if (res.convertedCount === 0 && res.skippedCount > 0) {
      ElMessage.warning(`没有新文件生成：${res.skippedCount} 个已跳过。若需覆盖已有 ${formatExt.value}，请勾选「覆盖已有文件」`);
    } else {
      ElMessage.success(`已转换 ${res.convertedCount} 个文件为 ${formatLabel.value}（${formatExt.value}）`);
    }
  } catch (err: any) {
    ElMessage.error(err.message);
  } finally {
    loading.value = false;
  }
}

function handleClear() {
  sourceDir.value = '';
  outputDir.value = '';
  sameDir.value = true;
  recursive.value = true;
  overwrite.value = false;
  outputFormat.value = 'html';
  fileList.value = [];
  selectedPaths.value = [];
  previewFile.value = '';
  previewHtml.value = '';
  previewTitle.value = '';
  lastResult.value = null;
}
</script>

<template>
  <div class="page confluence-page">
    <div class="page-header">
      <div class="page-header-row">
        <div>
          <h2>Confluence 格式转换</h2>
          <p class="hint">
            将语雀导出的 Markdown 转为 Word / Markdown / PDF。推荐选 Word 粘贴到 Confluence；
            右侧预览仅供参考。{{ formatHint }}
          </p>
        </div>
        <ClearCacheButton module="confluence" @cleared="handleClear" />
      </div>
    </div>

    <el-alert
      type="warning"
      :closable="false"
      show-icon
      class="disclaimer-alert"
      title="使用须知"
      :description="YUQUE_DISCLAIMER_SHORT"
    />

    <el-alert
      v-if="!serverReady"
      type="error"
      :closable="false"
      show-icon
      class="server-alert"
      :title="serverHint"
    />

    <div class="confluence-layout">
      <div class="confluence-panel">
        <div class="field">
          <label>Markdown 源目录</label>
          <div class="path-row">
            <FavoritePathInput v-model="sourceDir" placeholder="选择语雀导出的文件夹" />
            <el-button v-if="sourceDir" @click="openFolder(sourceDir)">打开</el-button>
          </div>
        </div>

        <div class="field">
          <label>导出格式</label>
          <el-radio-group v-model="outputFormat" class="format-group">
            <el-radio value="html">Confluence 网页 (.html)</el-radio>
            <el-radio value="docx">Word (.docx)</el-radio>
            <el-radio value="md">Markdown (.md)</el-radio>
            <el-radio value="pdf">PDF (.pdf)</el-radio>
          </el-radio-group>
        </div>

        <div class="field">
          <label>输出位置</label>
          <el-checkbox v-model="sameDir">输出到源目录（同路径 {{ formatExt }}）</el-checkbox>
          <div v-if="!sameDir" class="path-row" style="margin-top:8px">
            <FavoritePathInput v-model="outputDir" :placeholder="`${formatLabel} 输出目录`" />
            <el-button v-if="outputDir" @click="openFolder(outputDir)">打开</el-button>
          </div>
        </div>

        <div class="field opts-row">
          <el-checkbox v-model="recursive">包含子文件夹</el-checkbox>
          <el-checkbox v-model="overwrite">覆盖已有 {{ formatExt }} 文件</el-checkbox>
          <p v-if="outputFormat === 'md' && sameDir" class="field-note">
            导出 Markdown 到源目录时，默认跳过原 .md 文件；需覆盖请勾选上方选项。
          </p>
        </div>

        <div v-if="fileList.length" class="field file-select-field">
          <div class="file-select-header">
            <label>选择导出文件</label>
            <span class="file-select-count">已选 {{ selectedCount }} / {{ fileList.length }}</span>
          </div>
          <div class="tree-toolbar">
            <el-button size="small" @click="selectAllFiles">全选</el-button>
            <el-button size="small" @click="selectNoneFiles">取消全选</el-button>
            <el-button size="small" @click="invertSelection">反选</el-button>
          </div>
          <el-tree
            ref="fileTreeRef"
            :data="fileTreeData"
            show-checkbox
            node-key="id"
            default-expand-all
            :props="{ label: 'label', children: 'children' }"
            class="file-tree"
            @check="onTreeCheck"
          />
        </div>

        <div class="action-row">
          <el-button :loading="loading" @click="scanFiles">扫描 .md 文件</el-button>
          <el-button
            type="primary"
            :loading="loading"
            :disabled="!selectedCount"
            @click="handleConvert"
          >
            {{ convertButtonText }}
          </el-button>
        </div>

        <div v-if="sourceDir && !loading && !fileList.length" class="file-empty-hint">
          未找到 Markdown 文件。语雀导出一般在子目录里，请确认已勾选「包含子文件夹」，或点击「扫描 .md 文件」重试。
        </div>

        <div v-if="fileList.length" class="file-count">
          共 {{ fileList.length }} 个 Markdown 文件，请在上方树中勾选要导出的项
        </div>

        <div v-if="lastResult" class="result-card">
          <el-tag type="success">成功 {{ lastResult.convertedCount }}</el-tag>
          <el-tag v-if="lastResult.skippedCount" type="info">跳过 {{ lastResult.skippedCount }}</el-tag>
          <el-tag v-if="lastResult.failedCount" type="danger">失败 {{ lastResult.failedCount }}</el-tag>
          <span class="result-tip">{{ resultTip }}</span>
          <el-button size="small" link type="primary" @click="openFolder(lastResult.outputDir)">
            打开输出目录
          </el-button>
        </div>

        <div v-if="lastResult?.failed?.length" class="result-errors">
          <div class="result-errors-title">失败详情</div>
          <div
            v-for="item in lastResult.failed"
            :key="item.relativePath"
            class="result-error-item"
          >
            <span class="result-error-path">{{ item.relativePath }}</span>
            <span class="result-error-msg">{{ item.message }}</span>
          </div>
        </div>

        <div v-if="lastResult?.skipped?.length && lastResult.convertedCount === 0" class="result-errors skipped-hint">
          <div class="result-errors-title">跳过原因</div>
          <p class="field-note">
            目标位置已存在同名 {{ formatExt }} 文件。勾选「覆盖已有 {{ formatExt }} 文件」后重新导出即可。
          </p>
        </div>
      </div>

      <div class="confluence-preview">
        <div class="preview-toolbar">
          <el-select
            v-model="previewFile"
            filterable
            :placeholder="fileList.length ? '选择预览文件' : '请先选择目录并扫描 .md 文件'"
            :disabled="!fileList.length"
            style="flex:1"
            @change="persistSettings"
          >
            <el-option
              v-for="f in fileList"
              :key="f.absolutePath"
              :label="f.relativePath"
              :value="f.absolutePath"
            />
          </el-select>
          <el-button :loading="loading" @click="handlePreview">预览转换</el-button>
        </div>
        <div v-if="previewTitle" class="preview-title">预览：{{ previewTitle }}</div>
        <iframe
          class="preview-frame"
          :srcdoc="previewSrcdoc"
          sandbox="allow-same-origin allow-scripts allow-popups"
          title="转换效果预览"
        />
      </div>
    </div>
  </div>
</template>

<style scoped lang="scss">
.confluence-page {
  padding: 24px;
  height: calc(100vh - 56px);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.page-header-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 20px;

  h2 { margin: 0 0 6px; }
  .hint { margin: 0; color: var(--text-muted); font-size: 14px; max-width: 720px; }
}

.server-alert {
  margin-bottom: 16px;
}

.confluence-layout {
  display: flex;
  gap: 20px;
  flex: 1;
  min-height: 0;
}

.confluence-panel {
  width: 440px;
  flex-shrink: 0;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 20px;
  overflow-y: auto;
}

.confluence-preview {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 12px;
  overflow: hidden;
}

.field {
  margin-bottom: 16px;
  label {
    display: block;
    font-size: 13px;
    color: var(--text-muted);
    margin-bottom: 6px;
  }
}

.path-row {
  display: flex;
  gap: 8px;
}

.opts-row {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.format-group {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 6px;
}

.field-note {
  margin: 4px 0 0;
  font-size: 12px;
  color: var(--text-muted);
  line-height: 1.5;
}

.file-select-field {
  margin-bottom: 12px;
}

.file-select-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;

  label {
    margin-bottom: 0;
  }
}

.file-select-count {
  font-size: 12px;
  color: var(--text-muted);
}

.tree-toolbar {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
  margin-bottom: 8px;
}

.file-tree {
  max-height: 280px;
  overflow: auto;
  padding: 8px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--surface-2);
  font-size: 13px;
}

.action-row {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  margin-bottom: 12px;
}

.file-count {
  font-size: 13px;
  color: var(--text-muted);
  margin-bottom: 12px;
}

.file-empty-hint {
  font-size: 13px;
  color: var(--warning);
  margin-bottom: 12px;
  line-height: 1.5;
}

.result-card {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  padding: 12px;
  background: var(--surface-2);
  border-radius: 8px;
}

.result-tip {
  font-size: 12px;
  color: var(--text-muted);
  flex: 1;
  min-width: 180px;
}

.result-errors {
  margin-top: 10px;
  padding: 10px 12px;
  border-radius: 8px;
  background: rgba(239, 68, 68, 0.08);
  border: 1px solid rgba(239, 68, 68, 0.2);
}

.result-errors.skipped-hint {
  background: rgba(245, 158, 11, 0.08);
  border-color: rgba(245, 158, 11, 0.25);
}

.result-errors-title {
  font-size: 12px;
  font-weight: 600;
  margin-bottom: 8px;
  color: var(--text-muted);
}

.result-error-item {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin-bottom: 8px;
  font-size: 12px;
}

.result-error-path {
  color: var(--text);
  word-break: break-all;
}

.result-error-msg {
  color: #dc2626;
  word-break: break-word;
}

.preview-toolbar {
  display: flex;
  gap: 8px;
  padding: 12px;
  border-bottom: 1px solid var(--border);
}

.preview-title {
  padding: 8px 12px;
  font-size: 13px;
  color: var(--text-muted);
  border-bottom: 1px solid var(--border);
}

.preview-frame {
  flex: 1;
  width: 100%;
  border: none;
  background: #fff;
}
</style>
