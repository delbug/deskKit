<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { ElMessage, ElMessageBox } from 'element-plus';
import YuqueExportTree from '@/components/YuqueExportTree.vue';
import YuqueExportSelectTree from '@/components/YuqueExportSelectTree.vue';
import ClearCacheButton from '@/components/ClearCacheButton.vue';
import FavoritePathInput from '@/components/FavoritePathInput.vue';
import FavoriteUrlInput from '@/components/FavoriteUrlInput.vue';
import {
  exportYuque,
  exportYuqueBatch,
  cancelYuqueExport,
  resetYuqueExport,
  importYuqueProgress,
  pickSaveFile,
  pickOpenFile,
  writeTextFile,
  readTextFile,
  fetchYuqueExportProgress,
  openFolder,
  pickFolder,
  previewYuque,
  previewYuqueBook,
  type YuqueExportFormat,
} from '@/api';
import {
  buildDocTree,
  filterDocsBySlugs,
  formatProgressBar,
  mergeDocProgress,
  type ExportProgressDetail,
} from '@/utils/yuque-doc-tree';
import {
  buildYuqueSnapshot,
  parseYuqueSnapshot,
  snapshotToProgress,
  type YuqueExportOrder,
} from '@/utils/yuque-snapshot';
import { saveYuqueProgress, loadYuqueProgress } from '@/utils/appStorage';
import {
  YUQUE_DISCLAIMER_CONFIRM,
  YUQUE_DISCLAIMER_LINES,
  YUQUE_DISCLAIMER_TITLE,
} from '@/constants/yuque-disclaimer';

const YUQUE_DISCLAIMER_DISMISSED_KEY = 'yuque-disclaimer-dismissed';

const showDisclaimerCard = ref(
  sessionStorage.getItem(YUQUE_DISCLAIMER_DISMISSED_KEY) !== '1',
);

function dismissDisclaimerCard() {
  showDisclaimerCard.value = false;
  try {
    sessionStorage.setItem(YUQUE_DISCLAIMER_DISMISSED_KEY, '1');
  } catch {
    /* ignore */
  }
}

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
const exportOrder = ref<YuqueExportOrder>('top-down');
const selectedSlugs = ref<string[]>([]);
const selectTreeRef = ref<InstanceType<typeof YuqueExportSelectTree>>();
const delayMode = ref<'none' | 'fixed' | 'random'>('random');
const delayFixedSec = ref(5);
const delayMinSec = ref(3);
const delayMaxSec = ref(30);
const batchLimitEnabled = ref(false);
const batchLimitCount = ref(10);
const autoRoundEnabled = ref(false);
const autoRoundCount = ref(5);
const longDelayMinMin = ref(5);
const longDelayMaxMin = ref(60);
const autoRoundActive = ref(false);
const autoRoundPhase = ref<'exporting' | 'waiting' | 'paused' | null>(null);
const currentRound = ref(0);
const longDelayRemainingSec = ref(0);
const autoRoundStopRequested = ref(false);
const longDelaySkipRequested = ref(false);
const userExportPaused = ref(false);
const autoRoundSessionPending = ref(false);
const autoRoundPendingWait = ref(false);
const autoRoundNextRound = ref(1);
let longDelayTimer: ReturnType<typeof setInterval> | null = null;
let longDelayWaitResolve: ((action: 'continue' | 'skip' | 'stop' | 'paused') => void) | null = null;
const uiCountdownTick = ref(0);
let uiCountdownTimer: ReturnType<typeof setInterval> | null = null;
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
const actionFeedback = ref<{ kind: 'success' | 'warning' | 'error' | 'info'; text: string } | null>(null);
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
  if (autoRoundPhase.value === 'waiting') return '等待下一轮…';
  if (autoRoundPhase.value === 'paused' && autoRoundPendingWait.value) return '继续导出';
  if (batchExportRunning.value) {
    if (autoRoundActive.value) return `导出中 · 第 ${currentRound.value}/${autoRoundCount.value} 轮`;
    return '导出中…';
  }
  if (autoRoundSessionPending.value && autoRoundEnabled.value) return '继续导出';
  if (exportProgress.value && resumeExport.value) return '继续导出';
  return autoRoundEnabled.value ? '开始自动多轮' : '开始批量导出';
});

const exportStrategy = computed({
  get(): 'continuous' | 'batch_pause' | 'auto_round' {
    if (autoRoundEnabled.value) return 'auto_round';
    if (batchLimitEnabled.value) return 'batch_pause';
    return 'continuous';
  },
  set(v: 'continuous' | 'batch_pause' | 'auto_round') {
    autoRoundEnabled.value = v === 'auto_round';
    batchLimitEnabled.value = v === 'batch_pause';
  },
});

const delayModeHint = computed(() => {
  if (delayMode.value === 'none') return '未设置篇间间隔（易触发限流）';
  if (delayMode.value === 'fixed') return `每篇固定等待 ${delayFixedSec.value} 秒`;
  return `每篇随机 ${delayMinSec.value}~${delayMaxSec.value} 秒`;
});

interface ExportStatusHub {
  phaseLabel: string;
  phaseTag: 'exporting' | 'waiting' | 'paused';
  title: string;
  countdownSec: number;
  countdownLabel: string | null;
  detail: string | null;
  paused: boolean;
  showPause: boolean;
  showSkip: boolean;
  showStop: boolean;
  hint: string | null;
}

const exportStatusHub = computed((): ExportStatusHub | null => {
  void uiCountdownTick.value;

  if (autoRoundPhase.value === 'waiting') {
    return {
      phaseLabel: '轮间等待',
      phaseTag: 'waiting',
      title: `第 ${currentRound.value} / ${autoRoundCount.value} 轮已完成`,
      countdownSec: longDelayRemainingSec.value,
      countdownLabel: '距离下一轮',
      detail: `轮间随机 ${longDelayMinMin.value}~${longDelayMaxMin.value} 分钟`,
      paused: false,
      showPause: true,
      showSkip: true,
      showStop: true,
      hint: '程序在运行，请勿关闭窗口',
    };
  }

  if (autoRoundPhase.value === 'paused' && autoRoundPendingWait.value) {
    return {
      phaseLabel: '已暂停',
      phaseTag: 'paused',
      title: `轮间等待已暂停（第 ${currentRound.value} / ${autoRoundCount.value} 轮）`,
      countdownSec: longDelayRemainingSec.value,
      countdownLabel: '暂停前剩余',
      detail: '继续后将重新随机倒计时',
      paused: true,
      showPause: false,
      showSkip: false,
      showStop: false,
      hint: null,
    };
  }

  if (autoRoundSessionPending.value && autoRoundPhase.value === 'paused') {
    return {
      phaseLabel: '已暂停',
      phaseTag: 'paused',
      title: `第 ${autoRoundNextRound.value} / ${autoRoundCount.value} 轮导出已暂停`,
      countdownSec: 0,
      countdownLabel: null,
      detail: '点「继续导出」继续当前轮次',
      paused: true,
      showPause: false,
      showSkip: false,
      showStop: false,
      hint: null,
    };
  }

  if (batchExportRunning.value && docDelayRemainingSec.value > 0) {
    const roundPrefix = autoRoundActive.value
      ? `第 ${currentRound.value}/${autoRoundCount.value} 轮 · `
      : '';
    return {
      phaseLabel: '篇间等待',
      phaseTag: 'waiting',
      title: `${roundPrefix}等待导出下一篇`,
      countdownSec: docDelayRemainingSec.value,
      countdownLabel: '篇间间隔',
      detail: delayModeHint.value,
      paused: false,
      showPause: true,
      showSkip: false,
      showStop: autoRoundActive.value,
      hint: '程序在运行，请勿关闭窗口',
    };
  }

  if (batchExportRunning.value) {
    const exporting = progressStats.value.exporting;
    const roundPrefix = autoRoundActive.value
      ? `第 ${currentRound.value}/${autoRoundCount.value} 轮 · `
      : '';
    return {
      phaseLabel: '导出中',
      phaseTag: 'exporting',
      title: exporting
        ? `${roundPrefix}正在下载：${exporting.title}`
        : `${roundPrefix}批量导出进行中`,
      countdownSec: 0,
      countdownLabel: null,
      detail: `${progressStats.value.done + progressStats.value.duplicate}/${progressStats.value.total} 篇已处理`,
      paused: false,
      showPause: true,
      showSkip: false,
      showStop: autoRoundActive.value,
      hint: '暂停将在当前篇下载完成后生效',
    };
  }

  if (exportPaused.value && !autoRoundSessionPending.value) {
    return {
      phaseLabel: '已暂停',
      phaseTag: 'paused',
      title: batchLimitEnabled.value
        ? `本批已暂停，可继续导出最多 ${batchLimitCount.value} 篇`
        : '导出已暂停',
      countdownSec: 0,
      countdownLabel: null,
      detail: '可修改设置后点「继续导出」',
      paused: true,
      showPause: false,
      showSkip: false,
      showStop: false,
      hint: null,
    };
  }

  return null;
});

const exportSettingsLocked = computed(
  () =>
    batchMode.value
    && (batchExportRunning.value || autoRoundPhase.value === 'waiting')
    && !userExportPaused.value
    && autoRoundPhase.value !== 'paused',
);

function formatCountdownSec(totalSec: number): string {
  const sec = Math.max(0, Math.ceil(totalSec));
  if (sec <= 0) return '0 秒';
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  const s = sec % 60;
  if (h > 0) return `${h} 时 ${m} 分 ${String(s).padStart(2, '0')} 秒`;
  if (m > 0) return `${m} 分 ${String(s).padStart(2, '0')} 秒`;
  return `${s} 秒`;
}

const docDelayRemainingSec = computed(() => {
  void uiCountdownTick.value;
  const until = progressDetail.value?.delayUntil;
  if (!until) return 0;
  return Math.max(0, Math.ceil((new Date(until).getTime() - Date.now()) / 1000));
});

function syncUiCountdownTimer() {
  const longDelayActive =
    autoRoundPhase.value === 'waiting'
    || (autoRoundPhase.value === 'paused' && autoRoundPendingWait.value);
  const docDelayActive = batchExportRunning.value && !!progressDetail.value?.delayUntil;
  const need = longDelayActive || docDelayActive;
  if (need && !uiCountdownTimer) {
    uiCountdownTimer = setInterval(() => {
      uiCountdownTick.value += 1;
    }, 1000);
  } else if (!need && uiCountdownTimer) {
    clearInterval(uiCountdownTimer);
    uiCountdownTimer = null;
  }
}

watch(
  [batchExportRunning, autoRoundPhase, autoRoundPendingWait, () => progressDetail.value?.delayUntil],
  syncUiCountdownTimer,
  { immediate: true },
);

const docProgressList = computed(() => mergeDocProgress(bookCatalog.value, progressDetail.value));

const docTree = computed(() => buildDocTree(docProgressList.value));

const effectiveDocList = computed(() => {
  if (exportOrder.value !== 'custom' || !batchMode.value) return docProgressList.value;
  return filterDocsBySlugs(docProgressList.value, new Set(selectedSlugs.value));
});

const progressStats = computed(() => {
  const list = effectiveDocList.value;
  const total = list.length || progressDetail.value?.total || bookPreview.value?.total || 0;
  const done = list.length
    ? list.filter((d) => d.status === 'done').length
    : (progressDetail.value?.completed ?? 0);
  const failed = list.filter((d) => d.status === 'failed').length;
  const duplicate = list.filter((d) => d.status === 'duplicate').length;
  const exporting = list.find((d) => d.status === 'exporting');
  return {
    total,
    done,
    failed,
    duplicate,
    remaining: Math.max(0, total - done - duplicate),
    exporting,
  };
});

const progressBarText = computed(() =>
  formatProgressBar(progressStats.value.done + progressStats.value.duplicate, progressStats.value.total),
);

const showProgressPanel = computed(() => batchMode.value && docProgressList.value.length > 0);

const showCatalogHint = computed(
  () => batchMode.value && bookCatalog.value.length > 0 && !exportProgress.value && !exportingActive.value,
);

const hasExportProgress = computed(
  () => !!(exportProgress.value || progressDetail.value?.found),
);

const exportPaused = computed(
  () =>
    progressDetail.value?.status === 'paused'
    || pauseRequested.value
    || userExportPaused.value
    || autoRoundPhase.value === 'paused'
    || autoRoundSessionPending.value,
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

  const savedExportOrder = localStorage.getItem('yuque-export-order');
  if (savedExportOrder === 'top-down' || savedExportOrder === 'bottom-up' || savedExportOrder === 'custom') {
    exportOrder.value = savedExportOrder;
  }

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

  const savedBatchLimitEnabled = localStorage.getItem('yuque-batch-limit-enabled');
  if (savedBatchLimitEnabled === '1') batchLimitEnabled.value = true;
  else if (savedBatchLimitEnabled === '0') batchLimitEnabled.value = false;

  const savedBatchLimitCount = Number(localStorage.getItem('yuque-batch-limit-count'));
  if (Number.isFinite(savedBatchLimitCount) && savedBatchLimitCount >= 1) {
    batchLimitCount.value = Math.floor(savedBatchLimitCount);
  }

  if (localStorage.getItem('yuque-auto-round-enabled') === '1') autoRoundEnabled.value = true;
  const savedAutoRoundCount = Number(localStorage.getItem('yuque-auto-round-count'));
  if (Number.isFinite(savedAutoRoundCount) && savedAutoRoundCount >= 1) {
    autoRoundCount.value = Math.floor(savedAutoRoundCount);
  }
  const savedLongMin = Number(localStorage.getItem('yuque-long-delay-min'));
  if (Number.isFinite(savedLongMin) && savedLongMin >= 1) longDelayMinMin.value = Math.floor(savedLongMin);
  const savedLongMax = Number(localStorage.getItem('yuque-long-delay-max'));
  if (Number.isFinite(savedLongMax) && savedLongMax >= 1) longDelayMaxMin.value = Math.floor(savedLongMax);
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
  localStorage.setItem('yuque-export-order', exportOrder.value);
  localStorage.setItem('yuque-export-md', exportMd.value ? '1' : '0');
  localStorage.setItem('yuque-export-html', exportHtml.value ? '1' : '0');
  localStorage.setItem('yuque-batch-limit-enabled', batchLimitEnabled.value ? '1' : '0');
  localStorage.setItem('yuque-batch-limit-count', String(batchLimitCount.value));
  localStorage.setItem('yuque-auto-round-enabled', autoRoundEnabled.value ? '1' : '0');
  localStorage.setItem('yuque-auto-round-count', String(autoRoundCount.value));
  localStorage.setItem('yuque-long-delay-min', String(longDelayMinMin.value));
  localStorage.setItem('yuque-long-delay-max', String(longDelayMaxMin.value));
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
  stopLongDelayTimer();
  if (uiCountdownTimer) {
    clearInterval(uiCountdownTimer);
    uiCountdownTimer = null;
  }
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
      const p = data.progress;
      if (p?.exportOrder === 'top-down' || p?.exportOrder === 'bottom-up' || p?.exportOrder === 'custom') {
        exportOrder.value = p.exportOrder;
      }
      if (p?.selectedSlugs?.length) {
        selectedSlugs.value = p.selectedSlugs;
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

function formatErr(err: unknown): string {
  if (err instanceof Error && err.message.trim()) return err.message.trim();
  if (typeof err === 'string' && err.trim()) return err.trim();
  return '操作失败，请检查链接、Token 与网络后重试';
}

function notifyYuque(
  kind: 'success' | 'warning' | 'error' | 'info',
  message: string,
  opts?: { toast?: boolean; persist?: boolean },
) {
  const text = message.trim();
  if (!text) return;
  if (opts?.persist !== false) {
    actionFeedback.value = { kind, text };
  }
  if (opts?.toast === false) return;
  const payload = { message: text.replace(/\n+/g, ' '), duration: kind === 'error' ? 10000 : 8000, showClose: true };
  if (kind === 'success') ElMessage.success(payload);
  else if (kind === 'warning') ElMessage.warning(payload);
  else if (kind === 'info') ElMessage.info(payload);
  else ElMessage.error(payload);
}

const RATE_LIMIT_HINT =
  '语雀 API 请求过于频繁，请等待 5~10 分钟后再点「预览知识库」或「继续导出」。已有进度可从本地记录恢复，无需重新拉目录。';

function isRateLimitMessage(msg: string): boolean {
  return /too many|rate.?limit|过于频繁|429/i.test(msg);
}

/** 批量预览前的链接与认证校验 */
function validateBatchPreviewInput(url: string): string | null {
  if (authMode.value === 'share') {
    return shareLinkBatchIssue(url);
  }
  try {
    const u = new URL(/^https?:\/\//i.test(url) ? url : `https://${url}`);
    if (!u.hostname.includes('yuque.com')) {
      return '链接不是语雀地址，请填写形如 yuque.com/用户/知识库 的知识库链接';
    }
    const parts = u.pathname.split('/').filter(Boolean);
    if (parts.length < 2) {
      return '知识库链接格式不正确，需要至少包含「用户/知识库」，例如 yuque.com/your-name/your-repo';
    }
    return null;
  } catch {
    return '链接格式无效，请检查是否完整粘贴';
  }
}

async function loadBookCatalog(silent = false) {
  const url = shareUrl.value.trim();
  if (!url) {
    if (!silent) notifyYuque('warning', '请粘贴语雀链接');
    return false;
  }
  if (!batchMode.value) {
    if (!silent) notifyYuque('warning', '请先切换到「批量导出整个知识库」模式');
    return false;
  }
  const token = authMode.value === 'token' ? yuqueToken.value.trim() : '';
  if (authMode.value === 'token' && !token) {
    if (!silent) notifyYuque('warning', '请填写语雀 Token（头像 → 设置 → Token 中创建）');
    return false;
  }
  const inputIssue = validateBatchPreviewInput(url);
  if (inputIssue) {
    notifyYuque('error', inputIssue, { toast: !silent });
    return false;
  }
  if (catalogLoading) {
    if (!silent) notifyYuque('info', '正在加载知识库目录，请稍候…');
    return false;
  }
  catalogLoading = true;
  try {
    if (!silent) loading.value = true;
    const data = await previewYuqueBook(url, token || undefined);
    bookPreview.value = data;
    bookCatalog.value = data.docs ?? [];
    if (!selectedSlugs.value.length) {
      selectedSlugs.value = bookCatalog.value.map((d) => d.slug);
    }
    await refreshProgressDetail();
    if (!bookCatalog.value.length) {
      notifyYuque('warning', `已连接知识库「${data.bookName}」，但未找到可导出的文档。`, { toast: !silent });
      return true;
    }
    if (!silent) {
      persistAuthSettings();
      notifyYuque(
        'success',
        `知识库「${data.bookName}」共 ${data.total} 篇文档（${data.authMode === 'token' ? 'API' : '分享链接'}）`,
      );
    }
    return true;
  } catch (err: unknown) {
    const msg = formatErr(err);
    if (isRateLimitMessage(msg)) {
      notifyYuque('warning', RATE_LIMIT_HINT, { toast: !silent });
    } else {
      notifyYuque('error', msg || '预览知识库失败，请检查链接、Token 与网络', { toast: !silent });
    }
    return false;
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
    exportOrder,
    batchLimitEnabled,
    batchLimitCount,
    autoRoundEnabled,
    autoRoundCount,
    longDelayMinMin,
    longDelayMaxMin,
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
  exportOrder.value = 'top-down';
  selectedSlugs.value = [];
  autoRoundEnabled.value = false;
  autoRoundCount.value = 5;
  longDelayMinMin.value = 5;
  longDelayMaxMin.value = 60;
  autoRoundActive.value = false;
  autoRoundPhase.value = null;
  currentRound.value = 0;
  longDelayRemainingSec.value = 0;
  autoRoundStopRequested.value = false;
  userExportPaused.value = false;
  autoRoundSessionPending.value = false;
  autoRoundPendingWait.value = false;
  autoRoundNextRound.value = 1;
  stopLongDelayTimer();
  longDelayWaitResolve = null;
  stopProgressPolling();
}

async function handlePauseExport() {
  const url = shareUrl.value.trim();
  const dir = saveDir.value.trim();
  if (!url || !dir) return;

  if (autoRoundPhase.value === 'waiting') {
    userExportPaused.value = true;
    pauseLongDelayCountdown();
    autoRoundPhase.value = 'paused';
    autoRoundSessionPending.value = true;
    autoRoundPendingWait.value = true;
    autoRoundNextRound.value = currentRound.value + 1;
    resolveLongDelayWait('paused');
    batchExportRunning.value = false;
    stopProgressPolling();
    ElMessage.info('倒计时已暂停，点「继续导出」将从头开始等待');
    return;
  }

  userExportPaused.value = true;
  pauseRequested.value = true;
  try {
    await cancelYuqueExport(url, dir);
    ElMessage.info(
      autoRoundActive.value
        ? '正在暂停自动多轮，将在当前篇完成后停止…'
        : '正在暂停，将在当前篇下载完成后停止…',
    );
  } catch (err: any) {
    pauseRequested.value = false;
    userExportPaused.value = false;
    ElMessage.error(err.message || '暂停失败');
  }
}

async function handleExportSnapshot() {
  const url = shareUrl.value.trim();
  if (!url) return ElMessage.warning('请先填写语雀链接');
  if (!bookCatalog.value.length) {
    await loadBookCatalog(true);
  }
  if (!bookCatalog.value.length) {
    return ElMessage.warning('请先预览知识库以获取目录结构');
  }
  const safeName = progressBookName.value.replace(/[^\w\u4e00-\u9fff-]+/g, '_') || 'yuque';
  const picked = await pickSaveFile(`${safeName}-deskit-snapshot.json`);
  if (picked.cancelled || !picked.path) return;
  try {
    const dir = saveDir.value.trim();
    const snapshot = buildYuqueSnapshot({
      url,
      authMode: authMode.value,
      bookName: progressBookName.value,
      saveDir: dir,
      bookDir: progressDetail.value?.bookDir,
      exportOrder: exportOrder.value,
      selectedSlugs: exportOrder.value === 'custom' ? selectedSlugs.value : null,
      catalog: bookCatalog.value,
      docs: docProgressList.value,
      tree: docTree.value,
      progress: dir ? loadYuqueProgress(url, dir) : null,
    });
    await writeTextFile(picked.path, JSON.stringify(snapshot, null, 2));
    ElMessage.success('知识库结构快照已保存');
  } catch (err: any) {
    ElMessage.error(err.message || '导出快照失败');
  }
}

async function handleImportSnapshot() {
  const picked = await pickOpenFile();
  if (picked.cancelled || !picked.path) return;
  try {
    const content = await readTextFile(picked.path);
    const snapshot = parseYuqueSnapshot(content.content);
    shareUrl.value = snapshot.url;
    if (snapshot.authMode) authMode.value = snapshot.authMode;
    saveDir.value = snapshot.saveDir;
    exportOrder.value = snapshot.exportOrder;
    selectedSlugs.value = snapshot.selectedSlugs?.length
      ? snapshot.selectedSlugs
      : snapshot.catalog.map((d) => d.slug);
    bookCatalog.value = snapshot.catalog;
    bookPreview.value = {
      bookName: snapshot.bookName,
      total: snapshot.catalog.length,
      docs: snapshot.catalog,
    };
    const progress = snapshotToProgress(snapshot);
    saveYuqueProgress(snapshot.url, snapshot.saveDir, progress);
    await importYuqueProgress(snapshot.saveDir, progress);
    persistYuqueSettings();
    await refreshProgressDetail();
    ElMessage.success(`已导入「${snapshot.bookName}」结构快照（${snapshot.catalog.length} 篇）`);
  } catch (err: any) {
    ElMessage.error(err.message || '导入快照失败');
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
  actionFeedback.value = null;
  const url = shareUrl.value.trim();
  if (!url) {
    notifyYuque('warning', '请粘贴语雀分享链接');
    return;
  }
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
    notifyYuque('success', `已识别：${data.title}`);
  } catch (err: unknown) {
    const msg = formatErr(err);
    notifyYuque('error', msg || '预览识别失败，请检查分享链接是否有效');
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

function stopLongDelayTimer() {
  if (longDelayTimer) {
    clearInterval(longDelayTimer);
    longDelayTimer = null;
  }
}

function pauseLongDelayCountdown() {
  stopLongDelayTimer();
}

function resolveLongDelayWait(action: 'continue' | 'skip' | 'stop' | 'paused') {
  if (longDelayWaitResolve) {
    longDelayWaitResolve(action);
    longDelayWaitResolve = null;
  }
}

function randomLongDelaySec(minMin: number, maxMin: number): number {
  const min = Math.min(minMin, maxMin);
  const max = Math.max(minMin, maxMin);
  if (max <= min) return min * 60;
  return Math.floor(min * 60 + Math.random() * (max - min) * 60);
}

/** @param freshStart true=重新随机倒计时；false=沿用当前剩余秒数（暂停冻结后恢复用，继续导出时传 true） */
function waitBetweenAutoRounds(freshStart = true): Promise<'continue' | 'skip' | 'stop' | 'paused'> {
  if (freshStart) {
    longDelayRemainingSec.value = randomLongDelaySec(longDelayMinMin.value, longDelayMaxMin.value);
  }
  autoRoundPhase.value = 'waiting';
  longDelaySkipRequested.value = false;
  userExportPaused.value = false;

  return new Promise((resolve) => {
    longDelayWaitResolve = resolve;
    stopLongDelayTimer();
    longDelayTimer = setInterval(() => {
      if (userExportPaused.value) {
        pauseLongDelayCountdown();
        autoRoundPhase.value = 'paused';
        return;
      }
      if (autoRoundStopRequested.value) {
        stopLongDelayTimer();
        autoRoundPhase.value = null;
        longDelayRemainingSec.value = 0;
        resolveLongDelayWait('stop');
        return;
      }
      if (longDelaySkipRequested.value) {
        stopLongDelayTimer();
        autoRoundPhase.value = null;
        longDelayRemainingSec.value = 0;
        resolveLongDelayWait('skip');
        return;
      }
      longDelayRemainingSec.value = Math.max(0, longDelayRemainingSec.value - 1);
      if (longDelayRemainingSec.value <= 0) {
        stopLongDelayTimer();
        autoRoundPhase.value = null;
        resolveLongDelayWait('continue');
      }
    }, 1000);
  });
}

function skipLongDelayWait() {
  longDelaySkipRequested.value = true;
}

function stopAutoRound() {
  autoRoundStopRequested.value = true;
  userExportPaused.value = false;
  resolveLongDelayWait('stop');
  stopLongDelayTimer();
  autoRoundPhase.value = null;
  longDelayRemainingSec.value = 0;
  autoRoundSessionPending.value = false;
  autoRoundPendingWait.value = false;
}

function validateBatchExportOptions(): string | null {
  if (exportOrder.value === 'custom' && !selectedSlugs.value.length) {
    return '自定义导出请至少勾选一篇文档';
  }
  const useBatchLimit = batchLimitEnabled.value || autoRoundEnabled.value;
  if (useBatchLimit) {
    const limit = Math.floor(batchLimitCount.value);
    if (!Number.isFinite(limit) || limit < 1) {
      return '请填写每轮/每批导出篇数（至少 1 篇）';
    }
    if (!resumeExport.value) {
      return '分批或自动多轮导出需勾选「继续上次导出」';
    }
  }
  if (autoRoundEnabled.value) {
    const rounds = Math.floor(autoRoundCount.value);
    if (!Number.isFinite(rounds) || rounds < 1) {
      return '请填写自动多轮次数（至少 1 轮）';
    }
    const minM = Math.floor(longDelayMinMin.value);
    const maxM = Math.floor(longDelayMaxMin.value);
    if (!Number.isFinite(minM) || minM < 1 || !Number.isFinite(maxM) || maxM < 1) {
      return '请填写轮间等待时间（至少 1 分钟）';
    }
  }
  return null;
}

async function executeBatchExport(exportFormat: YuqueExportFormat) {
  const url = shareUrl.value.trim();
  const token = authMode.value === 'token' ? yuqueToken.value.trim() : '';
  const useBatchLimit = batchLimitEnabled.value || autoRoundEnabled.value;
  return exportYuqueBatch({
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
    exportOrder: exportOrder.value,
    selectedSlugs: exportOrder.value === 'custom' ? selectedSlugs.value : undefined,
    batchLimit: useBatchLimit ? Math.floor(batchLimitCount.value) : undefined,
  });
}

function storeBatchResult(result: Awaited<ReturnType<typeof exportYuqueBatch>>) {
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
}

function notifyBatchExportResult(
  result: Awaited<ReturnType<typeof exportYuqueBatch>>,
  exportFormat: YuqueExportFormat,
  opts?: { autoRound?: boolean; round?: number; totalRounds?: number; finalRound?: boolean },
) {
  if (opts?.autoRound && !opts.finalRound && result.batchLimitReached) {
    notifyYuque(
      'info',
      `第 ${opts.round}/${opts.totalRounds} 轮完成，本轮 ${result.newlyExported} 篇，累计 ${result.exported}/${result.total}，即将等待后继续…`,
      { toast: true },
    );
    return;
  }
  if (result.batchLimitReached) {
    notifyYuque(
      'success',
      `本批已导出 ${result.newlyExported} 篇（达到上限 ${Math.floor(batchLimitCount.value)} 篇），累计 ${result.exported}/${result.total}。请检查文件，确认无误后点「继续导出」再导下一批。`,
    );
  } else if (result.paused) {
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
}

async function runAutoRoundExport(exportFormat: YuqueExportFormat, startRound = 1) {
  const totalRounds = Math.floor(autoRoundCount.value);
  autoRoundActive.value = true;
  autoRoundStopRequested.value = false;
  if (startRound === 1 && !autoRoundSessionPending.value) {
    pauseRequested.value = false;
    userExportPaused.value = false;
  }

  for (let round = startRound; round <= totalRounds; round++) {
    if (autoRoundStopRequested.value) break;

    currentRound.value = round;
    autoRoundPhase.value = 'exporting';

    const result = await executeBatchExport(exportFormat);
    storeBatchResult(result);
    persistDelaySettings();
    persistAuthSettings();
    await refreshProgressDetail();
    pauseRequested.value = false;

    const isLastRound = round >= totalRounds;
    const allDone = result.remainingCount <= 0;

    if (autoRoundStopRequested.value || result.stoppedEarly) {
      notifyYuque('info', `自动多轮已停止（第 ${round}/${totalRounds} 轮），还剩 ${result.remainingCount} 篇`);
      break;
    }
    if (result.paused && !result.batchLimitReached) {
      autoRoundSessionPending.value = true;
      autoRoundPendingWait.value = false;
      autoRoundNextRound.value = round;
      userExportPaused.value = true;
      autoRoundPhase.value = 'paused';
      notifyYuque(
        'info',
        `第 ${round}/${totalRounds} 轮已暂停，点「继续导出」继续；若进入轮间等待将重新倒计时`,
      );
      break;
    }
    if (allDone) {
      notifyYuque('success', `自动多轮完成：全部 ${result.total} 篇已处理（共 ${round} 轮）`);
      break;
    }
    if (isLastRound) {
      notifyYuque(
        'success',
        `已完成 ${totalRounds} 轮，累计 ${result.exported}/${result.total}，还剩 ${result.remainingCount} 篇。可再点「继续导出」。`,
      );
      break;
    }

    notifyBatchExportResult(result, exportFormat, { autoRound: true, round, totalRounds, finalRound: false });

    const waitAction = await waitBetweenAutoRounds(true);
    if (waitAction === 'paused') {
      autoRoundSessionPending.value = true;
      autoRoundPendingWait.value = true;
      autoRoundNextRound.value = round + 1;
      notifyYuque(
        'info',
        `轮间等待已暂停（第 ${round}/${totalRounds} 轮已完成），点「继续导出」将从头开始倒计时`,
      );
      break;
    }
    if (waitAction === 'stop') {
      notifyYuque('info', `已停止自动多轮（第 ${round}/${totalRounds} 轮后），还剩 ${result.remainingCount} 篇`);
      break;
    }
  }

  if (!autoRoundSessionPending.value) {
    finishAutoRoundSession();
  }
}

function finishAutoRoundSession() {
  autoRoundActive.value = false;
  autoRoundPhase.value = null;
  currentRound.value = 0;
  longDelayRemainingSec.value = 0;
  autoRoundStopRequested.value = false;
  autoRoundSessionPending.value = false;
  autoRoundPendingWait.value = false;
  userExportPaused.value = false;
  pauseRequested.value = false;
}

async function resumeAutoRoundSession(exportFormat: YuqueExportFormat) {
  const totalRounds = Math.floor(autoRoundCount.value);
  const startRound = autoRoundNextRound.value;
  const needWaitFirst = autoRoundPendingWait.value;

  userExportPaused.value = false;
  pauseRequested.value = false;
  autoRoundStopRequested.value = false;
  autoRoundSessionPending.value = false;
  autoRoundPendingWait.value = false;
  batchExportRunning.value = true;
  autoRoundActive.value = true;
  startProgressPolling();

  if (needWaitFirst && startRound <= totalRounds) {
    const waitAction = await waitBetweenAutoRounds(true);
    if (waitAction === 'paused') {
      autoRoundSessionPending.value = true;
      autoRoundPendingWait.value = true;
      autoRoundNextRound.value = startRound;
      batchExportRunning.value = false;
      stopProgressPolling();
      return;
    }
    if (waitAction === 'stop') {
      notifyYuque('info', '自动多轮已停止');
      batchExportRunning.value = false;
      stopProgressPolling();
      finishAutoRoundSession();
      return;
    }
  }

  await runAutoRoundExport(exportFormat, startRound);
}

async function handleExport() {
  if (batchMode.value && batchExportRunning.value && !autoRoundSessionPending.value) return;
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
      if (exportOrder.value === 'custom' && !selectedSlugs.value.length) {
        ElMessage.warning('自定义导出请至少勾选一篇文档');
        stopProgressPolling();
        batchExportRunning.value = false;
        return;
      }
      const validationError = validateBatchExportOptions();
      if (validationError) {
        notifyYuque('warning', validationError);
        stopProgressPolling();
        batchExportRunning.value = false;
        return;
      }

      if (autoRoundEnabled.value) {
        if (autoRoundSessionPending.value) {
          await resumeAutoRoundSession(exportFormat);
        } else {
          autoRoundNextRound.value = 1;
          await runAutoRoundExport(exportFormat);
        }
        return;
      }

      const result = await executeBatchExport(exportFormat);
      persistDelaySettings();
      persistAuthSettings();
      storeBatchResult(result);
      await refreshProgressDetail();
      pauseRequested.value = false;
      notifyBatchExportResult(result, exportFormat);
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

    <div v-if="showDisclaimerCard" class="disclaimer-card" role="note">
      <button
        type="button"
        class="disclaimer-close"
        aria-label="关闭"
        @click="dismissDisclaimerCard"
      >
        <el-icon><Close /></el-icon>
      </button>
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

    <div class="command-strip">
      <div class="command-primary">
        <el-button :loading="loading && !batchExportRunning" @click="handlePreview">
          {{ batchMode ? '预览知识库' : '预览识别' }}
        </el-button>
        <el-button
          type="primary"
          :loading="loading"
          :disabled="batchExportRunning && !autoRoundSessionPending"
          @click="handleExport"
        >
          {{ exportLabel }}
        </el-button>
      </div>
      <el-button
        v-if="batchMode && hasExportProgress && !batchExportRunning && !exportStatusHub"
        type="danger"
        plain
        @click="handleRestartExport"
      >
        重新开始
      </el-button>
    </div>

    <div
      v-if="exportStatusHub"
      class="status-hub"
      :class="[exportStatusHub.phaseTag, { paused: exportStatusHub.paused }]"
    >
      <div class="status-hub-main">
        <div class="status-hub-text">
          <span class="status-phase-tag">{{ exportStatusHub.phaseLabel }}</span>
          <p class="status-title">{{ exportStatusHub.title }}</p>
          <p v-if="exportStatusHub.detail" class="status-detail">{{ exportStatusHub.detail }}</p>
          <p v-if="exportStatusHub.hint" class="status-hint">{{ exportStatusHub.hint }}</p>
        </div>
        <div
          v-if="exportStatusHub.countdownLabel && exportStatusHub.countdownSec > 0"
          class="status-countdown"
        >
          <span class="countdown-label">{{ exportStatusHub.countdownLabel }}</span>
          <strong class="countdown-value">{{ formatCountdownSec(exportStatusHub.countdownSec) }}</strong>
        </div>
      </div>
      <div
        v-if="exportStatusHub.showPause || exportStatusHub.showSkip || exportStatusHub.showStop"
        class="status-hub-actions"
      >
        <el-button
          v-if="exportStatusHub.showPause"
          type="warning"
          plain
          @click="handlePauseExport"
        >
          暂停
        </el-button>
        <el-button
          v-if="exportStatusHub.showSkip"
          plain
          @click="skipLongDelayWait"
        >
          跳过等待
        </el-button>
        <el-button
          v-if="exportStatusHub.showStop"
          type="danger"
          plain
          @click="stopAutoRound"
        >
          停止多轮
        </el-button>
      </div>
    </div>

    <el-alert
      v-if="actionFeedback"
      class="action-feedback"
      :title="actionFeedback.text"
      :type="actionFeedback.kind"
      show-icon
      :closable="actionFeedback.kind !== 'error'"
      @close="actionFeedback = null"
    />

    <p v-if="exportSettingsLocked" class="mode-tip lock-tip">
      导出进行中，参数已锁定。如需修改，请先暂停导出。
    </p>

    <div class="export-settings" :class="{ locked: exportSettingsLocked }">
      <section class="settings-section">
        <h3 class="settings-section-title">连接与目录</h3>
        <div class="field compact">
          <label>导出范围</label>
          <el-radio-group v-model="exportMode" class="mode-group">
            <el-radio value="single">单篇</el-radio>
            <el-radio value="batch">整个知识库</el-radio>
          </el-radio-group>
        </div>

        <template v-if="batchMode">
          <div class="field compact">
            <label>认证方式</label>
            <el-radio-group v-model="authMode" class="mode-group">
              <el-radio value="token">API Token（推荐）</el-radio>
              <el-radio value="share">分享链接</el-radio>
            </el-radio-group>
          </div>
          <div v-if="authMode === 'token'" class="token-block">
            <el-input
              v-model="yuqueToken"
              type="password"
              show-password
              placeholder="语雀个人 Token"
            />
            <p class="mode-tip">
              在
              <a href="https://www.yuque.com/settings/tokens" target="_blank" rel="noopener">语雀设置</a>
              中创建 Token，链接填知识库地址即可。
            </p>
          </div>
          <p v-else class="mode-tip">
            需粘贴知识库内任意一篇文档的分享链接（含文档 slug）。
          </p>
        </template>

        <div class="field">
          <div class="label-row">
            <label>{{ batchMode && authMode === 'token' ? '知识库链接' : '分享链接' }}</label>
            <el-button v-if="shareUrl" link type="danger" @click="clearShareUrl">清空</el-button>
          </div>
          <FavoriteUrlInput
            v-model="shareUrl"
            :placeholder="batchMode && authMode === 'token'
              ? 'https://www.yuque.com/your-name/your-repo'
              : 'https://www.yuque.com/用户/知识库/任意文档?singleDoc'"
          />
        </div>

        <div class="field row">
          <div class="grow">
            <label>保存目录</label>
            <FavoritePathInput v-model="saveDir" placeholder="选择或输入保存位置" />
          </div>
          <el-button v-if="saveDir" @click="openFolder(saveDir)">打开目录</el-button>
        </div>
      </section>

      <section v-if="batchMode" class="settings-section">
        <h3 class="settings-section-title">导出策略与间隔</h3>
        <p class="mode-tip warn strategy-warn">
          Token 短时间导出过多可能被语雀<strong>限流或临时屏蔽</strong>。请适当拉长等待时间，优先使用「随机间隔」和「自动多轮」。
        </p>

        <div class="field compact">
          <label>执行方式</label>
          <el-radio-group v-model="exportStrategy" class="strategy-group">
            <el-radio-button value="continuous">连续导出</el-radio-button>
            <el-radio-button value="batch_pause">分批暂停</el-radio-button>
            <el-radio-button value="auto_round">自动多轮</el-radio-button>
          </el-radio-group>
        </div>

        <div v-if="exportStrategy === 'batch_pause'" class="inline-inputs">
          <span>每批</span>
          <el-input-number v-model="batchLimitCount" :min="1" :max="9999" :step="1" />
          <span>篇后暂停，确认无误再继续</span>
        </div>

        <div v-if="exportStrategy === 'auto_round'" class="strategy-grid">
          <div class="inline-inputs">
            <span>每轮</span>
            <el-input-number v-model="batchLimitCount" :min="1" :max="9999" :step="1" />
            <span>篇</span>
          </div>
          <div class="inline-inputs">
            <span>共</span>
            <el-input-number v-model="autoRoundCount" :min="1" :max="999" :step="1" />
            <span>轮</span>
          </div>
          <div class="inline-inputs">
            <span>轮间等待</span>
            <el-input-number v-model="longDelayMinMin" :min="1" :max="600" :step="1" />
            <span>~</span>
            <el-input-number v-model="longDelayMaxMin" :min="1" :max="600" :step="1" />
            <span>分钟</span>
          </div>
        </div>

        <div class="delay-block">
          <label class="sub-label">篇间间隔（每篇完成后）</label>
          <el-radio-group v-model="delayMode" class="mode-group compact-radios">
            <el-radio value="random">随机（推荐）</el-radio>
            <el-radio value="fixed">固定</el-radio>
            <el-radio value="none">无间隔</el-radio>
          </el-radio-group>
          <div v-if="delayMode === 'fixed'" class="inline-inputs">
            <span>固定等待</span>
            <el-input-number v-model="delayFixedSec" :min="1" :max="120" :step="1" />
            <span>秒</span>
          </div>
          <div v-else-if="delayMode === 'random'" class="inline-inputs">
            <span>随机</span>
            <el-input-number v-model="delayMinSec" :min="1" :max="120" :step="1" />
            <span>~</span>
            <el-input-number v-model="delayMaxSec" :min="1" :max="120" :step="1" />
            <span>秒</span>
          </div>
        </div>
      </section>

      <el-collapse class="settings-collapse">
        <el-collapse-item title="格式与内容" name="format">
          <div class="field image-mode">
            <label>图片处理</label>
            <el-radio-group :model-value="imageMode" @change="onImageModeChange">
              <el-radio value="local">下载到本地 assets/</el-radio>
              <el-radio value="online">保留语雀在线链接</el-radio>
            </el-radio-group>
          </div>
          <div class="field">
            <el-checkbox v-model="standardMarkdown">标准 Markdown（整理标题/表格/图片）</el-checkbox>
          </div>
          <div class="field">
            <label>导出格式</label>
            <div class="format-checks">
              <el-checkbox v-model="exportMd">Markdown (.md)</el-checkbox>
              <el-checkbox v-model="exportHtml">Confluence 网页 (.html)</el-checkbox>
            </div>
            <p class="mode-tip">表格 / 数据表会自动导出为 Excel (.xlsx)，不受上方格式影响。</p>
          </div>
          <div class="field">
            <el-checkbox v-model="useDocFolder">
              {{ batchMode ? '每篇独立子文件夹' : '使用独立子文件夹' }}
            </el-checkbox>
          </div>
        </el-collapse-item>

        <el-collapse-item v-if="batchMode" title="导出顺序" name="order">
          <el-radio-group v-model="exportOrder" class="mode-group">
            <el-radio value="top-down">从上到下</el-radio>
            <el-radio value="bottom-up">从下到上</el-radio>
            <el-radio value="custom">自定义勾选</el-radio>
          </el-radio-group>
          <div v-if="exportOrder === 'custom' && docTree.length" class="select-tree-wrap">
            <div class="select-tree-actions">
              <el-button link type="primary" @click="selectTreeRef?.selectAll()">全选</el-button>
              <el-button link @click="selectTreeRef?.clearAll()">清空</el-button>
              <span class="muted">已选 {{ selectedSlugs.length }} / {{ docProgressList.length }} 篇</span>
            </div>
            <div class="tree-scroll select-tree-scroll">
              <YuqueExportSelectTree ref="selectTreeRef" v-model="selectedSlugs" :nodes="docTree" />
            </div>
          </div>
          <p v-else-if="exportOrder === 'custom'" class="mode-tip">
            请先点「预览知识库」加载目录后再勾选。
          </p>
        </el-collapse-item>

        <el-collapse-item v-if="batchMode" title="断点续导与快照" name="advanced">
          <el-checkbox v-model="resumeExport">继续上次导出（跳过已完成）</el-checkbox>
          <el-checkbox v-model="stopOnError">遇错暂停</el-checkbox>
          <div v-if="exportProgress && resumeExport" class="progress-card">
            <p>
              <strong>「{{ exportProgress.bookName }}」</strong>
              {{ exportProgress.completed }}/{{ exportProgress.total }} 篇
              <span v-if="exportProgress.remaining">，还剩 {{ exportProgress.remaining }} 篇</span>
            </p>
            <el-button v-if="exportProgress.bookDir" link type="primary" @click="openFolder(exportProgress.bookDir)">
              打开已导出目录
            </el-button>
          </div>
          <div class="snapshot-actions">
            <el-button @click="handleExportSnapshot">导出 JSON 快照</el-button>
            <el-button @click="handleImportSnapshot">导入 JSON 快照</el-button>
          </div>
        </el-collapse-item>
      </el-collapse>
    </div>

    <div v-if="showProgressPanel" class="progress-panel">
      <div class="progress-panel-head">
        <strong>{{ exportingActive ? '导出进度' : '知识库目录' }}：{{ progressBookName }}</strong>
        <span v-if="bookPreview?.total" class="catalog-total">共 {{ bookPreview.total }} 篇</span>
        <span v-if="exportingActive" class="live-tag">实时更新</span>
        <span v-else-if="exportPaused" class="live-tag paused-tag">已暂停</span>
      </div>
      <p v-if="showCatalogHint" class="catalog-hint">
        ✓ 已导出 · ≈ 已重复 · ✗ 失败 · ○ 待导出。开始导出后状态会实时更新。
      </p>
      <div class="progress-bar-line">{{ progressBarText }}</div>
      <div class="progress-stats-row">
        <span class="stat done">✓ {{ progressStats.done }} 已完成</span>
        <span v-if="progressStats.duplicate" class="stat duplicate">≈ {{ progressStats.duplicate }} 已重复</span>
        <span class="stat pending">○ {{ progressStats.remaining }} 待导出</span>
        <span v-if="progressStats.failed" class="stat failed">✗ {{ progressStats.failed }} 失败</span>
      </div>
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
  position: relative;
  margin-bottom: 20px;
  padding: 14px 36px 14px 16px;
  border-radius: 10px;
  border: 1px solid rgba(234, 179, 8, 0.35);
  background: rgba(234, 179, 8, 0.08);

  .disclaimer-close {
    position: absolute;
    top: 8px;
    right: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    transition: color 0.15s, background 0.15s;

    &:hover {
      color: var(--text);
      background: rgba(0, 0, 0, 0.06);
    }

    .el-icon {
      font-size: 14px;
    }
  }

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

.delay-inputs,
.batch-limit-inputs {
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

.auto-round-inputs {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-top: 8px;
}

.export-settings {
  &.locked {
    opacity: 0.62;
    pointer-events: none;
    user-select: none;
    filter: grayscale(0.15);
  }
}

.lock-tip {
  margin: -8px 0 12px;
  color: #fbbf24;
}

.action-feedback {
  margin-bottom: 12px;
  white-space: pre-wrap;
  word-break: break-word;

  :deep(.el-alert__title) {
    white-space: pre-wrap;
    line-height: 1.6;
  }
}

.actions {
  display: flex;
  gap: 10px;
  margin-bottom: 20px;
}

.command-strip {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
  margin-bottom: 16px;
  padding: 14px 16px;
  border-radius: 12px;
  border: 1px solid var(--border);
  background: var(--surface);

  :deep(.el-button:not(.is-link)) {
    min-width: 96px;
  }
}

.command-primary {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.yuque-page {
  :deep(.el-button:not(.is-link)) {
    height: 32px;
    padding: 8px 15px;
    font-size: 14px;
  }
}

.status-hub {
  margin-bottom: 16px;
  padding: 16px 18px;
  border-radius: 12px;
  border: 1px solid rgba(59, 130, 246, 0.35);
  background: rgba(59, 130, 246, 0.08);

  &.exporting {
    border-color: rgba(59, 130, 246, 0.35);
    background: rgba(59, 130, 246, 0.08);
  }

  &.waiting {
    border-color: rgba(34, 197, 94, 0.4);
    background: rgba(34, 197, 94, 0.08);
  }

  &.paused {
    border-color: rgba(245, 158, 11, 0.4);
    background: rgba(245, 158, 11, 0.08);
  }
}

.status-hub-main {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 20px;
  flex-wrap: wrap;
}

.status-hub-text {
  flex: 1;
  min-width: 200px;
}

.status-phase-tag {
  display: inline-block;
  margin-bottom: 6px;
  padding: 2px 8px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 600;
  background: rgba(255, 255, 255, 0.08);
}

.status-title {
  margin: 0 0 4px;
  font-size: 15px;
  font-weight: 600;
  line-height: 1.5;
}

.status-detail,
.status-hint {
  margin: 4px 0 0;
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-muted);
}

.status-countdown {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 4px;
  flex-shrink: 0;
}

.countdown-label {
  font-size: 12px;
  color: var(--text-muted);
}

.countdown-value {
  font-size: 28px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  line-height: 1.1;
  letter-spacing: -0.02em;
}

.status-hub-actions {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  margin-top: 12px;
  padding-top: 12px;
  border-top: 1px solid rgba(255, 255, 255, 0.08);

  :deep(.el-button) {
    min-width: 96px;
  }
}

.settings-section {
  margin-bottom: 16px;
  padding: 16px 18px;
  border-radius: 12px;
  border: 1px solid var(--border);
  background: var(--surface);
}

.settings-section-title {
  margin: 0 0 14px;
  font-size: 15px;
  font-weight: 600;
  color: var(--text);
}

.settings-collapse {
  margin-bottom: 16px;
  border: 1px solid var(--border);
  border-radius: 12px;
  overflow: hidden;
  --el-collapse-border-color: var(--border);
  --el-collapse-header-bg-color: var(--surface-2);
  --el-collapse-content-bg-color: var(--surface);
  --el-collapse-header-text-color: var(--text);
  --el-collapse-header-font-size: 14px;

  :deep(.el-collapse-item__header) {
    padding: 0 16px;
    height: 44px;
    font-weight: 600;
    font-size: 14px;
    color: var(--text) !important;
    background: var(--surface-2);
    border-bottom: 1px solid transparent;
    transition: background 0.15s, color 0.15s;

    &:hover {
      background: #2a3a52;
      color: var(--text) !important;
    }

    .el-collapse-item__arrow {
      color: var(--text-muted);
    }
  }

  :deep(.el-collapse-item.is-active > .el-collapse-item__header) {
    color: var(--text) !important;
    background: var(--surface-2);
    border-bottom-color: var(--border);
  }

  :deep(.el-collapse-item__wrap) {
    background: var(--surface);
    border-bottom: 1px solid var(--border);
  }

  :deep(.el-collapse-item:last-child .el-collapse-item__wrap) {
    border-bottom: none;
  }

  :deep(.el-collapse-item__content) {
    padding: 8px 16px 16px;
    color: var(--text);
  }
}

.strategy-group {
  display: flex;
  flex-wrap: wrap;
}

.strategy-grid {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-top: 12px;
}

.strategy-warn {
  margin-top: 0;
  margin-bottom: 12px;
}

.inline-inputs {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  margin-top: 10px;
  font-size: 13px;
  color: var(--text-muted);
}

.delay-block {
  margin-top: 14px;
  padding-top: 14px;
  border-top: 1px dashed var(--border);
}

.field.compact {
  margin-bottom: 12px;
}

.compact-radios {
  margin-top: 6px;
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
    &.duplicate { color: #fbbf24; }
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

.select-tree-wrap {
  margin-top: 10px;
}

.select-tree-actions {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 8px;
  font-size: 12px;
}

.select-tree-scroll {
  max-height: 320px;
}

.snapshot-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  margin-top: 8px;

  :deep(.el-button) {
    min-width: 96px;
  }
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
