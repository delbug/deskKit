<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { ElMessage } from 'element-plus';
import { deleteFiles, findDuplicates, formatSize, openFolder, pickFolder } from '@/api';
import ClearCacheButton from '@/components/ClearCacheButton.vue';
import type { DuplicateGroup } from '@/types';

const rootPath = ref('');
const loading = ref(false);
const groups = ref<DuplicateGroup[]>([]);
const stats = ref({ groupCount: 0, duplicateFiles: 0, wastedBytes: 0 });
const selected = ref<Set<string>>(new Set());

onMounted(() => {
  const saved = localStorage.getItem('dup-last-path');
  if (saved) rootPath.value = saved;
});

async function pickRoot() {
  try {
    const res = await pickFolder();
    if (!res.cancelled) rootPath.value = res.path;
  } catch (err: any) {
    ElMessage.error({ message: err.message, duration: 8000, showClose: true });
  }
}

async function scan() {
  if (!rootPath.value) return ElMessage.warning('请选择文件夹');
  localStorage.setItem('dup-last-path', rootPath.value);
  loading.value = true;
  selected.value = new Set();
  try {
    const res = await findDuplicates(rootPath.value);
    groups.value = res.groups;
    stats.value = res.stats;
    ElMessage.success(`找到 ${res.stats.groupCount} 组重复文件`);
  } catch (err: any) {
    ElMessage.error(err.message);
  } finally {
    loading.value = false;
  }
}

async function deleteSelected() {
  const paths = [...selected.value];
  if (!paths.length) return ElMessage.warning('请勾选要删除的重复项');
  loading.value = true;
  try {
    const res = await deleteFiles(paths.map((p) => ({ folderPath: rootPath.value, relativePath: p })));
    ElMessage.success(`已删除 ${res.deleted.length} 项`);
    await scan();
  } catch (err: any) {
    ElMessage.error(err.message);
  } finally {
    loading.value = false;
  }
}

function toggleFile(path: string, checked: boolean) {
  const s = new Set(selected.value);
  checked ? s.add(path) : s.delete(path);
  selected.value = s;
}

function handleClearDuplicates() {
  rootPath.value = '';
  groups.value = [];
  stats.value = { groupCount: 0, duplicateFiles: 0, wastedBytes: 0 };
  selected.value = new Set();
}
</script>

<template>
  <div class="page dup-page">
    <div class="dup-toolbar">
      <el-input v-model="rootPath" placeholder="扫描文件夹" style="flex:1;max-width:480px" />
      <el-button @click="pickRoot"><el-icon><Folder /></el-icon></el-button>
      <el-button type="primary" :loading="loading" @click="scan">扫描重复 (MD5)</el-button>
      <el-button v-if="rootPath" @click="openFolder(rootPath)">在 Finder 打开</el-button>
      <el-button type="danger" :disabled="!selected.size" @click="deleteSelected">删除选中 ({{ selected.size }})</el-button>
      <ClearCacheButton module="duplicates" @cleared="handleClearDuplicates" />
    </div>
    <div v-if="groups.length" class="dup-stats">
      <el-tag>{{ stats.groupCount }} 组重复</el-tag>
      <el-tag type="warning">{{ stats.duplicateFiles }} 个重复文件</el-tag>
      <el-tag type="danger">可释放 {{ formatSize(stats.wastedBytes) }}</el-tag>
    </div>
    <div class="dup-list">
      <div v-for="(g, i) in groups" :key="g.md5" class="dup-group">
        <div class="dup-group-head">
          <span>组 {{ i + 1 }}</span>
          <el-tag size="small">{{ g.count }} 个相同</el-tag>
          <span class="muted">{{ formatSize(g.size) }} · MD5 {{ g.md5.slice(0, 12) }}…</span>
        </div>
        <div v-for="f in g.files" :key="f.relativePath" class="dup-file">
          <el-checkbox :model-value="selected.has(f.relativePath)" @change="toggleFile(f.relativePath, !!$event)" />
          <span class="path-cell">{{ f.relativePath }}</span>
        </div>
      </div>
      <div v-if="!groups.length && !loading" class="empty-state">
        <el-icon><CopyDocument /></el-icon>
        <p>扫描文件夹中的 MD5 重复文件</p>
      </div>
    </div>
  </div>
</template>

<style scoped lang="scss">
.dup-page { padding: 20px; height: calc(100vh - 56px); display: flex; flex-direction: column; }
.dup-toolbar { display: flex; gap: 8px; flex-wrap: wrap; margin-bottom: 12px; }
.dup-stats { display: flex; gap: 8px; margin-bottom: 12px; }
.dup-list { flex: 1; overflow-y: auto; }
.dup-group {
  background: var(--surface); border: 1px solid var(--border); border-radius: 10px;
  margin-bottom: 12px; overflow: hidden;
}
.dup-group-head {
  display: flex; align-items: center; gap: 10px; padding: 10px 14px;
  background: var(--surface-2); font-size: 13px; font-weight: 600;
  .muted { font-weight: 400; color: var(--text-muted); font-size: 12px; }
}
.dup-file {
  display: flex; align-items: center; gap: 10px; padding: 8px 14px; border-top: 1px solid var(--border);
  font-size: 12px;
}
</style>
