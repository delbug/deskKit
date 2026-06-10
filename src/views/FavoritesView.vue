<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import { ElMessage, ElMessageBox } from 'element-plus';
import { openFolder, uid } from '@/api';
import ClearCacheButton from '@/components/ClearCacheButton.vue';
import { useConfig } from '@/composables/useConfig';
import type { FavoriteItem } from '@/types';

const router = useRouter();
const { config, load, persist, saveFavorite } = useConfig();
const editingId = ref<string | null>(null);
const editName = ref('');
const editNote = ref('');

const favorites = computed(() => config.value?.favorites || []);

onMounted(() => load());

async function applyFavorite(fav: FavoriteItem) {
  await persist({
    lastSession: {
      folders: fav.folders.map((f) => ({ ...f, id: f.id || uid() })),
      compareMode: config.value?.settings?.compareMode || 'md5',
    },
  });
  ElMessage.success(`已加载「${fav.name}」到对比页`);
  router.push('/compare');
}

async function removeFavorite(fav: FavoriteItem) {
  try {
    await ElMessageBox.confirm(`删除收藏「${fav.name}」？`, '确认');
    await saveFavorite('remove', { id: fav.id });
    await load();
    ElMessage.success('已删除');
  } catch { /* cancel */ }
}

function startEdit(fav: FavoriteItem) {
  editingId.value = fav.id;
  editName.value = fav.name;
  editNote.value = fav.note || '';
}

async function saveEdit(fav: FavoriteItem) {
  await saveFavorite('update', { ...fav, name: editName.value, note: editNote.value, updatedAt: new Date().toISOString() });
  editingId.value = null;
  await load();
  ElMessage.success('已更新');
}

async function handleClearFavorites() {
  editingId.value = null;
  await load();
}
</script>

<template>
  <div class="page favorites-page">
    <div class="page-header">
      <div class="page-header-row">
        <div>
          <h2>收藏管理</h2>
          <p>保存常用的文件夹组合，刷新页面后仍可从对比页或此处加载</p>
        </div>
        <ClearCacheButton module="favorites" @cleared="handleClearFavorites" />
      </div>
    </div>
    <div v-if="!favorites.length" class="empty-state">
      <el-icon><Star /></el-icon>
      <p>暂无收藏</p>
      <p class="hint">在「文件夹对比」页配置好路径后，点击星标按钮保存</p>
      <el-button type="primary" @click="router.push('/compare')">去对比页</el-button>
    </div>
    <div v-else class="fav-grid">
      <div v-for="fav in favorites" :key="fav.id" class="fav-card">
        <template v-if="editingId === fav.id">
          <el-input v-model="editName" placeholder="名称" />
          <el-input v-model="editNote" type="textarea" :rows="2" placeholder="备注" style="margin-top:8px" />
          <div class="fav-actions">
            <el-button size="small" type="primary" @click="saveEdit(fav)">保存</el-button>
            <el-button size="small" @click="editingId = null">取消</el-button>
          </div>
        </template>
        <template v-else>
          <div class="fav-title">
            <el-icon><Star /></el-icon>
            <strong>{{ fav.name }}</strong>
          </div>
          <p v-if="fav.note" class="fav-note">{{ fav.note }}</p>
          <ul class="fav-folders">
            <li v-for="(f, i) in fav.folders" :key="i">
              <span v-if="f.isPrimary" class="primary-badge">主</span>
              {{ f.label }}：<span class="path">{{ f.path || '未设置' }}</span>
            </li>
          </ul>
          <div class="fav-meta">{{ fav.folders.length }} 个文件夹 · {{ new Date(fav.createdAt).toLocaleString('zh-CN') }}</div>
          <div class="fav-actions">
            <el-button type="primary" size="small" @click="applyFavorite(fav)">应用到对比</el-button>
            <el-button size="small" @click="startEdit(fav)">编辑</el-button>
            <el-button size="small" @click="fav.folders[0]?.path && openFolder(fav.folders[0].path)">打开</el-button>
            <el-button size="small" type="danger" @click="removeFavorite(fav)">删除</el-button>
          </div>
        </template>
      </div>
    </div>
  </div>
</template>

<style scoped lang="scss">
.favorites-page { padding: 24px; overflow-y: auto; height: calc(100vh - 56px); }
.page-header { margin-bottom: 24px; h2 { margin: 0 0 6px; } p { margin: 0; color: var(--text-muted); font-size: 14px; } }
.page-header-row { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; }
.fav-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(340px, 1fr)); gap: 16px; }
.fav-card {
  background: var(--surface); border: 1px solid var(--border); border-radius: 12px; padding: 16px;
}
.fav-title { display: flex; align-items: center; gap: 8px; font-size: 16px; margin-bottom: 8px; }
.fav-note { font-size: 13px; color: var(--text-muted); margin: 0 0 8px; }
.fav-folders {
  list-style: none; padding: 0; margin: 0 0 10px; font-size: 12px;
  li { margin-bottom: 4px; .path { color: var(--text-muted); word-break: break-all; } }
}
.fav-meta { font-size: 11px; color: var(--text-muted); margin-bottom: 12px; }
.fav-actions { display: flex; flex-wrap: wrap; gap: 6px; }
.hint { font-size: 13px; color: var(--text-muted); }
</style>
