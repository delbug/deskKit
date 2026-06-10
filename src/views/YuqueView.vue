<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { ElMessage, ElMessageBox } from 'element-plus';
import YuqueExportTree from '@/components/YuqueExportTree.vue';
import ClearCacheButton from '@/components/ClearCacheButton.vue';
import {
  exportYuque,
  exportYuqueBatch,
  cancelYuqueExport,
  resetYuqueExport,
  fetchYuqueExportProgress,
  openFolder,
  pickFolder,
  previewYuque,
  previewYuqueBook,
  type YuqueExportFormat,
} from '@/api';
import {
  buildDocTree,
  formatProgressBar,
  mergeDocProgress,
  type ExportProgressDetail,
} from '@/utils/yuque-doc-tree';
import {
  YUQUE_DISCLAIMER_CONFIRM,
  YUQUE_DISCLAIMER_LINES,
  YUQUE_DISCLAIMER_TITLE,
} from '@/constants/yuque-disclaimer';

const shareUrl = ref('');
const saveDir = ref('');
const downloadImages = ref(true);
const imageMode = ref<'local' | 'online'>('local');
const standardMarkdown = ref(true);
const exportMd = ref(false);
const exportHtml = ref(true);
const exportMode = ref<'single' | 'batch'>('batch');
const authMode = ref<'share' | 'token'>('token');
const yuqueToken = ref('');
const batchMode = computed(() => exportMode.value === 'batch');
const useDocFolder = ref(false);
const stopOnError = ref(true);
const delayMode = ref<'none' | 'fixed' | 'random'>('random');
const delayFixedSec = ref(5);
const delayMinSec = ref(3);
const delayMaxSec = ref(30);
const loading = ref(false);
const preview = ref<{ title: string; preview: string; imageCount: number; charCount: number } | null>(null);
const bookPreview = ref<{ bookName: string; total: number; docs: { title: string; slug: string; dirPath: string }[] } | null>(null);
const bookCatalog = ref<{ slug: string; title: string; dirPath: string }[]>([]);
const progressDetail = ref<ExportProgressDetail | null>(null);
const exportingActive = ref(false);
const batchExportRunning = ref(false);
const pauseRequested = ref(false);
let progressPollTimer: ReturnType<typeof setInterval> | null = null;
const lastResult = ref<{ filePath: string; fileName: string; bookDir?: string } | null>(null);
const batchResult = ref<{
  exported: number;
  total: number;
  newlyExported: number;
  skippedCount: number;
  remainingCount: number;
  failedCount: number;
  bookDir: string;
  resume: boolean;
} | null>(null);
const resumeExport = ref(true);
const exportProgress = ref<{
  bookName: string;
  total: number;
  completed: number;
  remaining: number;
  failedCount: number;
  updatedAt?: string;
  bookDir: string;
} | null>(null);

const exportLabel = computed(() => {
  if (!batchMode.value) return '开始导出';
  if (exportProgress.value && resumeExport.value) return '继续导出';
  return '批量导出知识库';
});

const docProgressList = computed(() => mergeDocProgress(bookCatalog.value, progressDetail.value));

const docTree = computed(() => buildDocTree(docProgressList.value));

const progressStats = computed(() => {
  const list = docProgressList.value;
  const total = list.length || progressDetail.value?.total || bookPreview.value?.total || 0;
  const done = list.length
    ? list.filter((d) => d.status === 'done').length
    : (progressDetail.value?.completed ?? 0);
  const failed = list.filter((d) => d.status === 'failed').length;
  const exporting = list.find((d) => d.status === 'exporting');
  return {
    total,
    done,
    failed,
    remaining: Math.max(0, total - done),
    exporting,
  };
});

const progressBarText = computed(() => formatProgressBar(progressStats.value.done, progressStats.value.total));

const showProgressPanel = computed(() => batchMode.value && docProgressList.value.length > 0);

const showCatalogHint = computed(
  () => batchMode.value && bookCatalog.value.length > 0 && !exportProgress.value && !exportingActive.value,
);

const hasExportProgress = computed(
  () => !!(exportProgress.value || progressDetail.value?.found),
);

const exportPaused = computed(
  () => progressDetail.value?.status === 'paused' || pauseRequested.value,
);

const progressBookName = computed(
  () => bookPreview.value?.bookName || progressDetail.value?.bookName || '知识库',
);

function resolveExportFormat(): YuqueExportFormat | null {
  if (exportMd.value && exportHtml.value) return 'both';
  if (exportMd.value) return 'md';
  if (exportHtml.value) return 'html';
  return null;
}

function exportFormatLabel(format: YuqueExportFormat) {
  if (format === 'both') return 'Markdown + HTML';
  if (format === 'html') return 'Confluence 网页';
  return 'Markdown';
}

function loadYuqueSettings() {
  const savedUrl = localStorage.getItem('yuque-last-url');
  if (savedUrl) shareUrl.value = savedUrl;

  const savedDir = localStorage.getItem('yuque-save-dir');
  if (savedDir) saveDir.value = savedDir;

  const savedExportMode = localStorage.getItem('yuque-export-mode');
  if (savedExportMode === 'single' || savedExportMode === 'batch') {
    exportMode.value = savedExportMode;
  }

  const savedAuth = localStorage.getItem('yuque-auth-mode');
  if (savedAuth === 'share' || savedAuth === 'token') authMode.value = savedAuth;

  const savedToken = localStorage.getItem('yuque-token');
  if (savedToken) yuqueToken.value = savedToken;

  const savedImageMode = localStorage.getItem('yuque-image-mode');
  if (savedImageMode === 'online' || savedImageMode === 'local') {
    imageMode.value = savedImageMode;
    downloadImages.value = savedImageMode === 'local';
  }

  const savedStandard = localStorage.getItem('yuque-standard-markdown');
  if (savedStandard === '0') standardMarkdown.value = false;
  else if (savedStandard === '1') standardMarkdown.value = true;

  const savedDocFolder = localStorage.getItem('yuque-use-doc-folder');
  if (savedDocFolder === '1') useDocFolder.value = true;
  else if (savedDocFolder === '0') useDocFolder.value = false;

  const savedDelay = localStorage.getItem('yuque-delay-mode');
  if (savedDelay === 'none' || savedDelay === 'fixed' || savedDelay === 'random') {
    delayMode.value = savedDelay;
  }
  const n = (k: string, fallback: number) => {
    const v = Number(localStorage.getItem(k));
    return Number.isFinite(v) && v >= 0 ? v : fallback;
  };
  delayFixedSec.value = n('yuque-delay-fixed', 5);
  delayMinSec.value = n('yuque-delay-min', 3);
  delayMaxSec.value = n('yuque-delay-max', 30);

  const savedResume = localStorage.getItem('yuque-resume-export');
  if (savedResume === '0') resumeExport.value = false;
  else if (savedResume === '1') resumeExport.value = true;

  const savedStopOnError = localStorage.getItem('yuque-stop-on-error');
  if (savedStopOnError === '0') stopOnError.value = false;
  else if (savedStopOnError === '1') stopOnError.value = true;

  const savedExportMd = localStorage.getItem('yuque-export-md');
  const savedExportHtml = localStorage.getItem('yuque-export-html');
  if (savedExportMd === '1' || savedExportMd === '0') {
    exportMd.value = savedExportMd === '1';
  }
  if (savedExportHtml === '1' || savedExportHtml === '0') {
    exportHtml.value = savedExportHtml === '1';
  } else {
    const legacyFormat = localStorage.getItem('yuque-export-format');
    if (legacyFormat === 'md') {
      exportMd.value = true;
      exportHtml.value = false;
    } else if (legacyFormat === 'html') {
      exportMd.value = false;
      exportHtml.value = true;
    } else if (legacyFormat === 'both') {
      exportMd.value = true;
      exportHtml.value = true;
    } else if (localStorage.getItem('yuque-export-confluence-html') === '1') {
      exportMd.value = true;
      exportHtml.value = true;
    }
  }
}

function persistYuqueSettings() {
  const url = shareUrl.value.trim();
  if (url) localStorage.setItem('yuque-last-url', url);

  if (saveDir.value.trim()) {
    localStorage.setItem('yuque-save-dir', saveDir.value.trim());
  }

  localStorage.setItem('yuque-export-mode', exportMode.value);
  localStorage.setItem('yuque-auth-mode', authMode.value);
  if (yuqueToken.value.trim()) {
    localStorage.setItem('yuque-token', yuqueToken.value.trim());
  }
  localStorage.setItem('yuque-image-mode', imageMode.value);
  localStorage.setItem('yuque-standard-markdown', standardMarkdown.value ? '1' : '0');
  localStorage.setItem('yuque-use-doc-folder', useDocFolder.value ? '1' : '0');
  localStorage.setItem('yuque-delay-mode', delayMode.value);
  localStorage.setItem('yuque-delay-fixed', String(delayFixedSec.value));
  localStorage.setItem('yuque-delay-min', String(delayMinSec.value));
  localStorage.setItem('yuque-delay-max', String(delayMaxSec.value));
  localStorage.setItem('yuque-resume-export', resumeExport.value ? '1' : '0');
  localStorage.setItem('yuque-stop-on-error', stopOnError.value ? '1' : '0');
  localStorage.setItem('yuque-export-md', exportMd.value ? '1' : '0');
  localStorage.setItem('yuque-export-html', exportHtml.value ? '1' : '0');
}

onMounted(async () => {
  loadYuqueSettings();
  await refreshProgressDetail();
  if (batchMode.value && shareUrl.value.trim() && saveDir.value.trim() && !bookCatalog.value.length) {
    await loadBookCatalog(true);
  }
});

onUnmounted(() => {
  stopProgressPolling();
});

function startProgressPolling() {
  stopProgressPolling();
  exportingActive.value = true;
  refreshProgressDetail();
  progressPollTimer = setInterval(() => {
    refreshProgressDetail();
  }, 2000);
}

function stopProgressPolling() {
  exportingActive.value = false;
  if (progressPollTimer) {
    clearInterval(progressPollTimer);
    progressPollTimer = null;
  }
}

async function refreshProgressDetail() {
  const url = shareUrl.value.trim();
  const dir = saveDir.value.trim();
  if (!batchMode.value || !url || !dir) {
    progressDetail.value = null;
    exportProgress.value = null;
    return;
  }
  try {
    const token = authMode.value === 'token' ? yuqueToken.value.trim() : '';
    const data = await fetchYuqueExportProgress(url, dir, token || undefined);
    if (data.found) {
      progressDetail.value = data;
      if (data.docs?.length) {
        bookCatalog.value = data.docs.map((d) => ({
          slug: d.slug,
          title: d.title,
          dirPath: d.dirPath,
        }));
        if (!bookPreview.value && data.bookName) {
          bookPreview.value = {
            bookName: data.bookName,
            total: data.total ?? data.docs.length,
            docs: bookCatalog.value,
          };
        }
      }
      if (data.bookName && data.total != null && data.completed != null) {
        exportProgress.value = {
          bookName: data.bookName,
          total: data.total,
          completed: data.completed,
          remaining: data.remaining ?? Math.max(0, data.total - data.completed),
          failedCount: data.failedCount ?? 0,
          updatedAt: data.updatedAt,
          bookDir: data.bookDir || '',
        };
      }
    } else {
      progressDetail.value = null;
      exportProgress.value = null;
    }
  } catch {
    progressDetail.value = null;
  }
}

let catalogLoading = false;

async function loadBookCatalog(silent = false) {
  const url = shareUrl.value.trim();
  if (!url || !batchMode.value) return;
  const token = authMode.value === 'token' ? yuqueToken.value.trim() : '';
  if (authMode.value === 'token' && !token) {
    if (!silent) ElMessage.warning('请填写语雀 Token');
    return;
  }
  if (authMode.value === 'share' && shareLinkBatchIssue(url)) return;
  if (catalogLoading) return;
  catalogLoading = true;
  try {
    if (!silent) loading.value = true;
    const data = await previewYuqueBook(url, token || undefined);
    bookPreview.value = data;
    bookCatalog.value = data.docs;
    await refreshProgressDetail();
    if (!silent) {
      persistAuthSettings();
      ElMessage.success(`知识库「${data.bookName}」共 ${data.total} 篇文档（${data.authMode === 'token' ? 'API' : '分享链接'}）`);
    }
  } catch (err: any) {
    const msg = String(err.message || '');
    if (/too many/i.test(msg)) {
      ElMessage.warning({
        message: '语雀 API 请求过于频繁，请等待 5~10 分钟后再点「预览知识库」或「继续导出」。已有进度可从本地记录恢复，无需重新拉目录。',
        duration: 10000,
        showClose: true,
      });
    } else if (!silent) {
      ElMessage.error({ message: msg, duration: 8000, showClose: true });
    }
  } finally {
    catalogLoading = false;
    if (!silent) loading.value = false;
  }
}

watch(shareUrl, () => persistYuqueSettings());

watch(
  [
    saveDir,
    exportMode,
    authMode,
    yuqueToken,
    imageMode,
    standardMarkdown,
    useDocFolder,
    exportMd,
    exportHtml,
    delayMode,
    delayFixedSec,
    delayMinSec,
    delayMaxSec,
    resumeExport,
    stopOnError,
  ],
  () => persistYuqueSettings(),
);

watch([shareUrl, saveDir, authMode, exportMode], () => {
  refreshProgressDetail();
}, { flush: 'post' });

function persistAuthSettings() {
  persistYuqueSettings();
}

function persistDelaySettings() {
  persistYuqueSettings();
}

function onImageModeChange(mode: 'local' | 'online') {
  imageMode.value = mode;
  downloadImages.value = mode === 'local';
  persistYuqueSettings();
}

function clearShareUrl() {
  shareUrl.value = '';
  localStorage.removeItem('yuque-last-url');
  preview.value = null;
  bookPreview.value = null;
  bookCatalog.value = [];
  progressDetail.value = null;
  lastResult.value = null;
  batchResult.value = null;
  exportProgress.value = null;
  pauseRequested.value = false;
}

function handleClearYuque() {
  shareUrl.value = '';
  saveDir.value = '';
  downloadImages.value = true;
  imageMode.value = 'local';
  standardMarkdown.value = true;
  exportMode.value = 'batch';
  authMode.value = 'token';
  yuqueToken.value = '';
  useDocFolder.value = false;
  delayMode.value = 'random';
  delayFixedSec.value = 5;
  delayMinSec.value = 3;
  delayMaxSec.value = 30;
  resumeExport.value = true;
  stopOnError.value = true;
  preview.value = null;
  bookPreview.value = null;
  bookCatalog.value = [];
  progressDetail.value = null;
  exportingActive.value = false;
  lastResult.value = null;
  batchResult.value = null;
  exportProgress.value = null;
  pauseRequested.value = false;
  stopProgressPolling();
}

async function handlePauseExport() {
  const url = shareUrl.value.trim();
  const dir = saveDir.value.trim();
  if (!url || !dir) return;
  pauseRequested.value = true;
  try {
    await cancelYuqueExport(url, dir);
    ElMessage.info('正在暂停，将在当前篇下载完成后停止…');
  } catch (err: any) {
    pauseRequested.value = false;
    ElMessage.error(err.message || '暂停失败');
  }
}

async function handleRestartExport() {
  const url = shareUrl.value.trim();
  const dir = saveDir.value.trim();
  if (!url || !dir) return ElMessage.warning('请先填写链接并选择保存目录');
  try {
    await ElMessageBox.confirm(
      '将清除该知识库在本地的导出进度记录（不会删除已下载的文件）。重置后请修改选项，再点「批量导出知识库」从头开始。',
      '重新开始导出',
      { type: 'warning', confirmButtonText: '确认重置', cancelButtonText: '取消' },
    );
  } catch {
    return;
  }
  if (batchExportRunning.value) {
    await cancelYuqueExport(url, dir);
  }
  try {
    await resetYuqueExport(url, dir);
    batchResult.value = null;
    exportProgress.value = null;
    progressDetail.value = null;
    pauseRequested.value = false;
    resumeExport.value = false;
    await refreshProgressDetail();
    ElMessage.success('进度已重置，请修改选项后重新导出');
  } catch (err: any) {
    ElMessage.error(err.message || '重置失败');
  }
}

/** 分享链接模式下，仅有 /用户/知识库 时无法批量导出 */
function shareLinkBatchIssue(url: string): string | null {
  const raw = url.trim();
  if (!raw) return null;
  try {
    const u = new URL(/^https?:\/\//i.test(raw) ? raw : `https://${raw}`);
    if (!u.hostname.includes('yuque.com')) return null;
    const parts = u.pathname.split('/').filter(Boolean);
    if (parts[0] === 'docs') return null;
    if (parts.length === 2) {
      return `当前为「分享链接」模式，但链接只有 /${parts.join('/')}（缺少文档段）。\n\n请任选其一：\n1. 切换到「API Token」模式，填写 Token，继续用知识库链接\n2. 打开知识库内任意一篇文档 → 分享 → 复制带文档 slug 的链接`;
    }
    return null;
  } catch {
    return null;
  }
}

async function pickSaveDir() {
  const res = await pickFolder();
  if (!res.cancelled) {
    saveDir.value = res.path;
    persistYuqueSettings();
  }
}

async function handlePreview() {
  const url = shareUrl.value.trim();
  if (!url) return ElMessage.warning('请粘贴语雀分享链接');
  preview.value = null;
  lastResult.value = null;
  batchResult.value = null;
  if (batchMode.value) {
    await loadBookCatalog(false);
    return;
  }
  loading.value = true;
  bookPreview.value = null;
  try {
    const data = await previewYuque(url, standardMarkdown.value);
    preview.value = data;
    ElMessage.success(`已识别：${data.title}`);
  } catch (err: any) {
    ElMessage.error({ message: err.message, duration: 8000, showClose: true });
  } finally {
    loading.value = false;
  }
}

async function confirmYuqueDisclaimer(): Promise<boolean> {
  try {
    await ElMessageBox.confirm(YUQUE_DISCLAIMER_CONFIRM, YUQUE_DISCLAIMER_TITLE, {
      confirmButtonText: '我已阅读并同意',
      cancelButtonText: '取消',
      type: 'warning',
      customClass: 'yuque-disclaimer-box',
      dangerouslyUseHTMLString: false,
    });
    return true;
  } catch {
    return false;
  }
}

async function handleExport() {
  const url = shareUrl.value.trim();
  if (!url) return ElMessage.warning('请粘贴语雀分享链接');
  if (!saveDir.value) return ElMessage.warning('请先选择保存目录');
  const exportFormat = resolveExportFormat();
  if (!exportFormat) return ElMessage.warning('请至少勾选一种导出格式（Markdown 或 Confluence 网页）');
  if (!(await confirmYuqueDisclaimer())) return;
  loading.value = true;
  batchResult.value = null;
  try {
    if (batchMode.value) {
      await refreshProgressDetail();
      if (!bookCatalog.value.length) {
        await loadBookCatalog(true);
      }
      if (!bookCatalog.value.length) {
        ElMessage.warning('无法获取知识库目录，请先点「预览知识库」，或等待语雀限流解除后再试');
        stopProgressPolling();
        return;
      }
      batchExportRunning.value = true;
      pauseRequested.value = false;
      startProgressPolling();

      const token = authMode.value === 'token' ? yuqueToken.value.trim() : '';
      if (authMode.value === 'token' && !token) {
        ElMessage.warning('请填写语雀 Token');
        stopProgressPolling();
        batchExportRunning.value = false;
        return;
      }
      const shareIssue = authMode.value === 'share' ? shareLinkBatchIssue(url) : null;
      if (shareIssue) {
        ElMessage.error({ message: shareIssue, duration: 10000, showClose: true });
        stopProgressPolling();
        batchExportRunning.value = false;
        return;
      }
      const result = await exportYuqueBatch({
        url,
        saveDir: saveDir.value,
        token: token || undefined,
        resume: resumeExport.value,
        downloadImages: downloadImages.value,
        standardMarkdown: standardMarkdown.value,
        exportFormat,
        delayMode: delayMode.value,
        delayFixedSec: delayFixedSec.value,
        delayMinSec: delayMinSec.value,
        delayMaxSec: delayMaxSec.value,
        useDocFolder: useDocFolder.value,
        stopOnError: stopOnError.value,
      });
      persistDelaySettings();
      persistAuthSettings();
      batchResult.value = {
        exported: result.exported,
        total: result.total,
        newlyExported: result.newlyExported,
        skippedCount: result.skippedCount,
        remainingCount: result.remainingCount,
        failedCount: result.failedCount,
        bookDir: result.bookDir,
        resume: result.resume,
      };
      lastResult.value = { filePath: result.bookDir, fileName: result.bookName, bookDir: result.bookDir };
      await refreshProgressDetail();
      pauseRequested.value = false;
      if (result.paused) {
        ElMessage.success({
          message: `已暂停（已完成 ${result.exported}/${result.total}）。可修改选项后点「继续导出」。`,
          duration: 8000,
        });
      } else if (result.stoppedEarly) {
        ElMessage.warning({
          message: `导出因错误已暂停（已完成 ${result.exported}/${result.total}）。进度已保存，几小时后用相同链接和目录点「继续导出」即可从失败处重试。`,
          duration: 12000,
          showClose: true,
        });
      } else if (result.remainingCount > 0) {
        ElMessage.warning(
          `本次新导出 ${result.newlyExported} 篇，跳过 ${result.skippedCount} 篇；` +
          `累计 ${result.exported}/${result.total}，还剩 ${result.remainingCount} 篇`,
        );
      } else if (result.failedCount) {
        ElMessage.warning(`全部完成 ${result.exported}/${result.total}，${result.failedCount} 篇失败可再次继续导出重试`);
      } else if (result.resume && result.skippedCount) {
        ElMessage.success(`续导完成：新导出 ${result.newlyExported} 篇，共 ${result.exported}/${result.total} 篇`);
      } else {
        ElMessage.success(`已批量导出 ${result.exported} 篇（${exportFormatLabel(exportFormat)}）到「${result.bookName}」`);
      }
    } else {
      const result = await exportYuque({
        url,
        saveDir: saveDir.value,
        downloadImages: downloadImages.value,
        standardMarkdown: standardMarkdown.value,
        useDocFolder: useDocFolder.value,
        exportFormat,
      });
      lastResult.value = {
        filePath: result.filePath,
        fileName: result.fileName,
      };
      preview.value = {
        title: result.title,
        preview: '',
        imageCount: result.imageCount,
        charCount: result.charCount,
      };
      const parts = [
        exportMd.value && result.mdFileName ? result.mdFileName : '',
        exportHtml.value && result.htmlFileName ? result.htmlFileName : '',
      ].filter(Boolean);
      ElMessage.success(`已保存：${parts.join('、') || result.fileName}`);
    }
  } catch (err: any) {
    ElMessage.error({ message: err.message, duration: 8000, showClose: true });
    await refreshProgressDetail();
  } finally {
    loading.value = false;
    batchExportRunning.value = false;
    if (batchMode.value) {
      stopProgressPolling();
    }
  }
}
</script>

<template>
  <div class="page yuque-page">
    <div class="yuque-header">
      <div>
        <h2>语雀文档导出</h2>
        <p class="hint">
          批量导出支持两种方式：<strong>API Token</strong>（推荐，可填知识库链接）或<strong>分享链接</strong>（需文档级链接）。
        </p>
      </div>
      <ClearCacheButton module="yuque" :save-dir="saveDir" :url="shareUrl" @cleared="handleClearYuque" />
    </div>

    <div class="disclaimer-card" role="note">
      <p class="disclaimer-title">{{ YUQUE_DISCLAIMER_TITLE }}</p>
      <ul class="disclaimer-list">
        <li v-for="(line, i) in YUQUE_DISCLAIMER_LINES" :key="i">{{ line }}</li>
      </ul>
      <p class="disclaimer-foot">
        语雀官方协议：
        <a href="https://www.yuque.com/terms" target="_blank" rel="noopener">服务协议</a>
        ·
        <a href="https://www.yuque.com/privacy" target="_blank" rel="noopener">隐私权政策</a>
      </p>
    </div>

    <div v-if="batchMode" class="field export-mode-card">
      <label class="section-title">认证方式（批量导出）</label>
      <el-radio-group v-model="authMode" class="mode-group">
        <el-radio value="token">API Token（推荐）</el-radio>
        <el-radio value="share">分享链接（无需 Token）</el-radio>
      </el-radio-group>
      <div v-if="authMode === 'token'" class="token-block">
        <el-input
          v-model="yuqueToken"
          type="password"
          show-password
          placeholder="语雀个人 Token"
        />
        <p class="mode-tip">
          在语雀头像 → 设置 → Token 中创建：
          <a href="https://www.yuque.com/settings/tokens" target="_blank" rel="noopener">yuque.com/settings/tokens</a>。
          链接可填知识库地址，如 <code>yuque.com/your-name/your-repo</code>。
        </p>
      </div>
      <p v-else class="mode-tip">
        需粘贴知识库内<strong>任意一篇文档</strong>的分享链接，格式：
        <code>yuque.com/用户/知识库/文档slug?singleDoc</code>
      </p>
    </div>

    <div class="field">
      <div class="label-row">
        <label>{{ batchMode && authMode === 'token' ? '语雀知识库链接' : '语雀分享链接' }}</label>
        <el-button v-if="shareUrl" link type="danger" @click="clearShareUrl">清空</el-button>
      </div>
      <el-input
        v-model="shareUrl"
        type="textarea"
        :rows="3"
        :placeholder="batchMode && authMode === 'token'
          ? 'https://www.yuque.com/your-name/your-repo'
          : 'https://www.yuque.com/用户/知识库/任意文档?singleDoc'"
      />
    </div>

    <div class="field row">
      <div class="grow">
        <label>保存目录</label>
        <el-input v-model="saveDir" readonly placeholder="点击右侧按钮选择保存位置" />
      </div>
      <el-button @click="pickSaveDir">
        <el-icon><Folder /></el-icon>
        选择目录
      </el-button>
      <el-button v-if="saveDir" @click="openFolder(saveDir)">在 Finder 打开</el-button>
    </div>

    <div class="field export-mode-card">
      <label class="section-title">导出模式与下载间隔</label>
      <el-radio-group v-model="exportMode" class="mode-group">
        <el-radio value="single">单篇导出</el-radio>
        <el-radio value="batch">批量导出整个知识库</el-radio>
      </el-radio-group>

      <div class="batch-delay" :class="{ disabled: !batchMode }">
        <p v-if="!batchMode" class="mode-tip">切换到「批量导出」后可设置每篇之间的下载间隔。</p>
        <template v-else>
          <p class="sub-label">下载间隔（降低被限流风险）</p>
          <el-radio-group v-model="delayMode">
            <el-radio value="none">无间隔（最快，不推荐）</el-radio>
            <el-radio value="fixed">固定间隔</el-radio>
            <el-radio value="random">随机间隔（推荐）</el-radio>
          </el-radio-group>
          <div v-if="delayMode === 'fixed'" class="delay-inputs">
            <span>每篇完成后等待</span>
            <el-input-number v-model="delayFixedSec" :min="1" :max="120" :step="1" />
            <span>秒</span>
          </div>
          <div v-else-if="delayMode === 'random'" class="delay-inputs">
            <span>每篇完成后随机等待</span>
            <el-input-number v-model="delayMinSec" :min="1" :max="120" :step="1" />
            <span>~</span>
            <el-input-number v-model="delayMaxSec" :min="1" :max="120" :step="1" />
            <span>秒</span>
          </div>
          <p class="mode-tip">
            例如 88 篇、3~30 秒随机间隔，大约需要 4~44 分钟。间隔越长越稳妥。
          </p>
        </template>
      </div>
    </div>

    <div v-if="batchMode" class="field export-mode-card">
      <label class="section-title">断点续导</label>
      <el-checkbox v-model="resumeExport">继续上次导出（跳过已完成的文档）</el-checkbox>
      <el-checkbox v-model="stopOnError">遇错暂停（限流或单篇失败时停止，保存进度供稍后重试）</el-checkbox>
      <div v-if="exportProgress && resumeExport" class="progress-card">
        <p>
          <strong>「{{ exportProgress.bookName }}」</strong>
          已完成 {{ exportProgress.completed }}/{{ exportProgress.total }} 篇
          <span v-if="exportProgress.remaining">，还剩 {{ exportProgress.remaining }} 篇</span>
        </p>
        <p v-if="exportProgress.failedCount" class="warn">上次有 {{ exportProgress.failedCount }} 篇失败，继续导出时会从第一篇失败处重试</p>
        <p v-if="exportProgress.updatedAt" class="muted">进度记录于 {{ new Date(exportProgress.updatedAt).toLocaleString() }}</p>
        <el-button v-if="exportProgress.bookDir" link type="primary" @click="openFolder(exportProgress.bookDir)">
          打开已导出目录
        </el-button>
      </div>
      <p v-else class="mode-tip">
        每完成一篇自动记录进度（应用内 + 保存目录下的 <code>.deskit-yuque-*.json</code>）。
        用<strong>相同知识库链接和保存目录</strong>再次打开，目录树会显示 ✓ 已导出 / ✗ 失败 / ○ 待导出。
      </p>
    </div>

    <div class="field image-mode">
      <label>图片处理方式</label>
      <el-radio-group :model-value="imageMode" @change="onImageModeChange">
        <el-radio value="local">下载到本地 assets/（适合离线备份、本地阅读）</el-radio>
        <el-radio value="online">保留语雀在线链接（适合再导入语雀）</el-radio>
      </el-radio-group>
      <p v-if="imageMode === 'local'" class="mode-tip warn">
        Markdown 里会是 <code>![](assets/xxx.png)</code> 这类本地路径。再导入语雀时，图片<strong>不会</strong>自动上传，需手动处理或改用「保留在线链接」。
      </p>
      <p v-else class="mode-tip">
        Markdown 里保留 <code>![](https://cdn.nlark.com/...)</code>。导入语雀时图片通常能正常显示（需原图链接仍可访问）。
      </p>
    </div>

    <div class="field">
      <el-checkbox v-model="standardMarkdown">导出为标准 Markdown（去除颜色标签、整理标题/表格/图片格式）</el-checkbox>
    </div>

    <div class="field export-mode-card">
      <label class="section-title">导出文件格式（可多选）</label>
      <div class="format-checks">
        <el-checkbox v-model="exportMd">Markdown (.md)</el-checkbox>
        <el-checkbox v-model="exportHtml">Confluence 网页 (.html)</el-checkbox>
      </div>
      <p class="mode-tip">
        导入 Confluence：勾选 HTML，导出后打开同目录的 <strong>-confluence.docx</strong>（Word/WPS）→ 全选复制 → 粘贴（图片最稳）。不要用 .md 直接粘贴。
      </p>
      <p v-if="exportHtml && imageMode === 'online'" class="mode-tip warn">
        保留语雀在线图片时，HTML 会尝试下载 CDN 图片内嵌；若部分失败，可改用「下载到本地 assets/」再导出。
      </p>
    </div>

    <div class="field">
      <el-checkbox v-model="useDocFolder">
        {{ batchMode ? '每篇使用独立子文件夹（标题/标题.md + assets/）' : '单篇也使用独立文件夹（标题/标题.md + assets/）' }}
      </el-checkbox>
      <p v-if="batchMode && !useDocFolder" class="mode-tip">
        默认按语雀目录<strong>从上到下</strong>逐篇导出，文件直接保存在对应分组目录下（如 <code>分组名/文章标题.md</code>），不会每篇单独建一层文件夹。
      </p>
    </div>

    <div class="actions">
      <el-button :loading="loading && !batchExportRunning" @click="handlePreview">
        {{ batchMode ? '预览知识库' : '预览识别' }}
      </el-button>
      <el-button type="primary" :loading="loading" :disabled="batchExportRunning" @click="handleExport">
        {{ exportLabel }}
      </el-button>
      <el-button
        v-if="batchMode && batchExportRunning"
        type="warning"
        plain
        @click="handlePauseExport"
      >
        暂停导出
      </el-button>
      <el-button
        v-if="batchMode && hasExportProgress && !batchExportRunning"
        type="danger"
        plain
        @click="handleRestartExport"
      >
        重新开始
      </el-button>
    </div>
    <p v-if="batchMode && batchExportRunning" class="mode-tip pause-tip">
      暂停会在<strong>当前这篇</strong>下载完成后生效，进度会自动保存。
    </p>
    <p v-else-if="batchMode && exportPaused" class="mode-tip pause-tip">
      导出已暂停。修改格式、图片、间隔等选项后，勾选「继续上次导出」并点「继续导出」即可。
    </p>

    <div v-if="showProgressPanel" class="progress-panel">
      <div class="progress-panel-head">
        <strong>{{ exportingActive ? '导出进度' : '知识库目录' }}：{{ progressBookName }}</strong>
        <span v-if="bookPreview?.total" class="catalog-total">共 {{ bookPreview.total }} 篇</span>
        <span v-if="exportingActive" class="live-tag">实时更新中</span>
        <span v-else-if="exportPaused" class="live-tag paused-tag">已暂停</span>
      </div>
      <p v-if="showCatalogHint" class="catalog-hint">
        下方为知识库全部文档。✓ 已导出 · ✗ 失败 · ○ 待导出。点「批量导出知识库」后状态会实时更新。
      </p>
      <div class="progress-bar-line">{{ progressBarText }}</div>
      <div class="progress-stats-row">
        <span class="stat done">✓ {{ progressStats.done }} 已完成</span>
        <span class="stat pending">○ {{ progressStats.remaining }} 待导出</span>
        <span v-if="progressStats.failed" class="stat failed">✗ {{ progressStats.failed }} 失败</span>
      </div>
      <p v-if="progressStats.exporting" class="current-doc">
        正在下载：<strong>{{ progressStats.exporting.title }}</strong>
      </p>
      <div class="tree-scroll">
        <YuqueExportTree :nodes="docTree" />
      </div>
      <p class="structure-hint">
        导出目录：<code>{{ progressDetail?.bookDir || `${saveDir || '保存目录'}/${progressBookName}` }}</code>
      </p>
    </div>

    <div v-if="preview && !batchMode" class="preview-card">
      <div class="preview-head">
        <strong>{{ preview.title }}</strong>
        <span>{{ preview.charCount.toLocaleString() }} 字 · {{ preview.imageCount }} 张图片</span>
      </div>
      <pre v-if="preview.preview" class="preview-body">{{ preview.preview }}</pre>
    </div>

    <div v-if="batchResult" class="result-card">
      <el-icon><CircleCheck /></el-icon>
      <span>
        累计 {{ batchResult.exported }}/{{ batchResult.total }} 篇
        <template v-if="batchResult.newlyExported">（本次 +{{ batchResult.newlyExported }}）</template>
      </span>
      <span v-if="batchResult.remainingCount" class="warn">，还剩 {{ batchResult.remainingCount }} 篇</span>
      <span v-if="batchResult.failedCount" class="warn">（{{ batchResult.failedCount }} 篇失败）</span>
      <el-button link type="primary" @click="openFolder(batchResult.bookDir)">打开导出目录</el-button>
    </div>

    <div v-else-if="lastResult && !batchMode" class="result-card">
      <el-icon><CircleCheck /></el-icon>
      <span>已导出：<code>{{ lastResult.fileName }}</code></span>
      <el-button link type="primary" @click="openFolder(saveDir)">打开保存目录</el-button>
    </div>
  </div>
</template>

<style scoped lang="scss">
.yuque-page {
  padding: 20px 24px;
  overflow: auto;
  height: 100%;
}

.yuque-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 16px;
}

.disclaimer-card {
  margin-bottom: 20px;
  padding: 14px 16px;
  border-radius: 10px;
  border: 1px solid rgba(234, 179, 8, 0.35);
  background: rgba(234, 179, 8, 0.08);

  .disclaimer-title {
    margin: 0 0 8px;
    font-size: 13px;
    font-weight: 700;
    color: #ca8a04;
  }

  .disclaimer-list {
    margin: 0;
    padding-left: 18px;
    font-size: 12px;
    line-height: 1.65;
    color: var(--text-muted);

    li + li {
      margin-top: 4px;
    }
  }

  .disclaimer-foot {
    margin: 10px 0 0;
    font-size: 12px;
    color: var(--text-muted);

    a {
      color: #60a5fa;
    }
  }
}

h2 {
  margin: 0 0 8px;
  font-size: 20px;
}

.hint {
  margin: 0;
  color: var(--text-muted);
  font-size: 13px;
  line-height: 1.6;

  code {
    padding: 1px 6px;
    border-radius: 4px;
    background: var(--surface-2);
    font-size: 12px;
  }
}

.field {
  margin-bottom: 16px;

  label {
    display: block;
    margin-bottom: 6px;
    font-size: 13px;
    color: var(--text-muted);
  }

  .label-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 6px;

    label {
      margin-bottom: 0;
    }
  }

  &.row {
    display: flex;
    align-items: flex-end;
    gap: 10px;
    flex-wrap: wrap;
  }

  &.export-mode-card {
    padding: 14px 16px;
    background: var(--surface);
    border: 1px solid var(--primary);
    border-radius: 10px;
  }

  &.image-mode {
    padding: 12px 14px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
  }

  &.batch-delay {
    margin-top: 14px;
    padding-top: 14px;
    border-top: 1px solid var(--border);

    &.disabled {
      opacity: 0.85;
    }
  }
}

.section-title {
  display: block;
  margin-bottom: 10px;
  font-size: 14px;
  font-weight: 600;
  color: var(--text);
}

.sub-label {
  margin: 0 0 8px;
  font-size: 13px;
  color: var(--text-muted);
}

.format-checks {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 20px;
}

.mode-group {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 16px;
}

.token-block {
  margin-top: 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.hint a {
  color: #93c5fd;
}

.delay-inputs {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 10px;
  font-size: 13px;
  color: var(--text-muted);
  flex-wrap: wrap;
}

.mode-tip {
  margin: 10px 0 0;
  font-size: 12px;
  line-height: 1.6;
  color: var(--text-muted);

  code {
    padding: 1px 5px;
    border-radius: 4px;
    background: var(--surface-2);
    font-size: 11px;
  }

  &.warn {
    color: #fbbf24;
  }
}

.grow {
  flex: 1;
  min-width: 280px;
}

.actions {
  display: flex;
  gap: 10px;
  margin-bottom: 20px;
}

.preview-card {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 14px 16px;
  margin-bottom: 12px;
}

.preview-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 10px;
  font-size: 13px;

  span {
    color: var(--text-muted);
    white-space: nowrap;
  }
}

.doc-list {
  margin: 0 0 12px;
  padding-left: 20px;
  font-size: 13px;
  line-height: 1.8;
  max-height: 200px;
  overflow: auto;

  .doc-path {
    color: var(--text-muted);
    font-size: 12px;
  }

  .muted {
    color: var(--text-muted);
    list-style: none;
    margin-left: -20px;
  }
}

.structure-hint {
  margin: 0;
  font-size: 12px;
  color: var(--text-muted);

  code {
    font-size: 11px;
    word-break: break-all;
  }
}

.preview-body {
  margin: 0;
  max-height: 320px;
  overflow: auto;
  padding: 12px;
  background: var(--surface-2);
  border-radius: 8px;
  font-size: 12px;
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-word;
}

.progress-panel {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 14px 16px;
  margin-bottom: 12px;
}

.progress-panel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 10px;
  font-size: 14px;

  .live-tag {
    font-size: 12px;
    color: #60a5fa;
    animation: pulse 1.5s ease-in-out infinite;

    &.paused-tag {
      color: #f59e0b;
      animation: none;
    }
  }

  .catalog-total {
    font-size: 12px;
    color: var(--text-muted);
    font-weight: 400;
  }
}

.catalog-hint {
  font-size: 12px;
  color: var(--text-muted);
  margin: -4px 0 10px;
}

.pause-tip {
  margin: -4px 0 12px;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}

.progress-bar-line {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 14px;
  letter-spacing: 0.02em;
  margin-bottom: 8px;
  color: #22c55e;
}

.progress-stats-row {
  display: flex;
  flex-wrap: wrap;
  gap: 12px 16px;
  margin-bottom: 10px;
  font-size: 12px;

  .stat {
    &.done { color: #22c55e; }
    &.pending { color: var(--text-muted); }
    &.failed { color: #ef4444; }
  }
}

.current-doc {
  margin: 0 0 10px;
  font-size: 12px;
  color: var(--text-muted);
}

.tree-scroll {
  max-height: 420px;
  overflow: auto;
  padding: 8px 10px;
  background: var(--surface-2);
  border-radius: 8px;
  margin-bottom: 10px;
}

.progress-card {
  margin-top: 10px;
  padding: 10px 12px;
  background: var(--surface-2);
  border-radius: 8px;
  font-size: 13px;
  line-height: 1.6;

  p {
    margin: 0 0 6px;
  }

  .warn {
    color: var(--warning);
  }

  .muted {
    color: var(--text-muted);
    font-size: 12px;
  }
}

.result-card {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 14px;
  background: rgba(34, 197, 94, 0.1);
  border: 1px solid rgba(34, 197, 94, 0.3);
  border-radius: 8px;
  font-size: 13px;
  flex-wrap: wrap;

  .el-icon {
    color: var(--success);
    font-size: 18px;
  }

  .warn {
    color: var(--warning);
  }

  code {
    font-size: 12px;
  }
}
</style>
