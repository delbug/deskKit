<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import { ElMessage, ElMessageBox } from 'element-plus';
import { openFolder, pickFolder } from '@/api';
import ClearCacheButton from '@/components/ClearCacheButton.vue';
import FavoritePathInput from '@/components/FavoritePathInput.vue';
import FavoriteUrlInput from '@/components/FavoriteUrlInput.vue';
import { useFavorites } from '@/composables/useFavorites';
import { usePathUrlFavorites } from '@/composables/usePathUrlFavorites';
import type { CompareMode, FavoriteItem, PathFavorite, UrlFavorite } from '@/types';

const router = useRouter();
const activeTab = ref('path');
const pathForm = ref({ name: '', path: '', note: '' });
const urlForm = ref({ name: '', url: '', note: '' });
const editingPathId = ref<string | null>(null);
const editingUrlId = ref<string | null>(null);

const {
  pathFavorites,
  urlFavorites,
  reload: reloadPathUrl,
  addPathFavorite,
  addUrlFavorite,
  updatePathFavorite,
  updateUrlFavorite,
  removePathFavorite,
  removeUrlFavorite,
} = usePathUrlFavorites();

const {
  favorites: compareFavorites,
  load: loadCompare,
  applyFavorite,
  removeFavorite: removeCompareFavorite,
} = useFavorites();

onMounted(async () => {
  await reloadPathUrl();
  await loadCompare();
});

async function submitPath() {
  if (!pathForm.value.path.trim()) return ElMessage.warning('请填写路径');
  try {
    if (editingPathId.value) {
      await updatePathFavorite({
        id: editingPathId.value,
        name: pathForm.value.name.trim() || pathForm.value.path.split('/').pop() || '文件夹',
        path: pathForm.value.path.trim(),
        note: pathForm.value.note.trim() || undefined,
        createdAt: pathFavorites.value.find((p) => p.id === editingPathId.value)?.createdAt || new Date().toISOString(),
      });
      editingPathId.value = null;
      ElMessage.success('已更新');
    } else {
      await addPathFavorite(pathForm.value.path, pathForm.value.name, pathForm.value.note);
      ElMessage.success('收藏成功');
    }
    pathForm.value = { name: '', path: '', note: '' };
  } catch (err: any) {
    const msg = err.message || '保存失败';
    if (msg.includes('已在收藏')) ElMessage.warning(msg);
    else ElMessage.error(msg);
  }
}

async function submitUrl() {
  if (!urlForm.value.url.trim()) return ElMessage.warning('请填写网址');
  try {
    if (editingUrlId.value) {
      await updateUrlFavorite({
        id: editingUrlId.value,
        name: urlForm.value.name.trim() || '网址',
        url: urlForm.value.url.trim(),
        note: urlForm.value.note.trim() || undefined,
        createdAt: urlFavorites.value.find((u) => u.id === editingUrlId.value)?.createdAt || new Date().toISOString(),
      });
      editingUrlId.value = null;
      ElMessage.success('已更新');
    } else {
      await addUrlFavorite(urlForm.value.url, urlForm.value.name, urlForm.value.note);
      ElMessage.success('收藏成功');
    }
    urlForm.value = { name: '', url: '', note: '' };
  } catch (err: any) {
    const msg = err.message || '保存失败';
    if (msg.includes('已在收藏')) ElMessage.warning(msg);
    else ElMessage.error(msg);
  }
}

function editPath(item: PathFavorite) {
  editingPathId.value = item.id;
  pathForm.value = { name: item.name, path: item.path, note: item.note || '' };
  activeTab.value = 'path';
}

function editUrl(item: UrlFavorite) {
  editingUrlId.value = item.id;
  urlForm.value = { name: item.name, url: item.url, note: item.note || '' };
  activeTab.value = 'url';
}

async function deletePath(item: PathFavorite) {
  try {
    await ElMessageBox.confirm(`删除地址「${item.name}」？`, '确认', { type: 'warning' });
    await removePathFavorite(item.id);
  } catch { /* cancel */ }
}

async function deleteUrl(item: UrlFavorite) {
  try {
    await ElMessageBox.confirm(`删除网址「${item.name}」？`, '确认', { type: 'warning' });
    await removeUrlFavorite(item.id);
  } catch { /* cancel */ }
}

async function pickPathForm() {
  const res = await pickFolder();
  if (!res.cancelled) {
    pathForm.value.path = res.path;
    if (!pathForm.value.name) pathForm.value.name = res.name || '';
  }
}

function handleClearAll() {
  editingPathId.value = null;
  editingUrlId.value = null;
  pathForm.value = { name: '', path: '', note: '' };
  urlForm.value = { name: '', url: '', note: '' };
  reloadPathUrl();
  loadCompare();
}

function compareModeLabel(mode?: CompareMode) {
  return mode === 'name' ? '仅路径' : 'MD5 内容';
}

async function onApplyCompare(fav: FavoriteItem, autoCompare = false) {
  await applyFavorite(fav, { autoCompare });
  ElMessage.success(`已加载对比组合「${fav.name}」`);
}
</script>

<template>
  <div class="page favorites-page">
    <div class="page-header">
      <div class="page-header-row">
        <div>
          <h2>收藏管理</h2>
          <p>分别管理<strong>本地地址</strong>与<strong>网址</strong>。各页面的路径/链接输入框聚焦时可下拉选用。</p>
        </div>
        <ClearCacheButton module="favorites" @cleared="handleClearAll" />
      </div>
    </div>

    <el-tabs v-model="activeTab" class="fav-tabs">
      <el-tab-pane label="地址" name="path">
        <div class="form-card">
          <h3>{{ editingPathId ? '编辑地址' : '添加地址' }}</h3>
          <div class="form-grid">
            <el-input v-model="pathForm.name" placeholder="名称（可选）" />
            <FavoritePathInput v-model="pathForm.path" placeholder="文件夹路径" :show-save="false" />
            <el-input v-model="pathForm.note" placeholder="备注（可选）" />
          </div>
          <div class="form-actions">
            <el-button type="primary" @click="submitPath">{{ editingPathId ? '保存修改' : '添加' }}</el-button>
            <el-button v-if="editingPathId" @click="editingPathId = null; pathForm = { name: '', path: '', note: '' }">取消</el-button>
            <el-button @click="pickPathForm">选择文件夹</el-button>
          </div>
        </div>
        <div v-if="!pathFavorites.length" class="empty-inline">暂无地址收藏</div>
        <div v-else class="item-list">
          <div v-for="item in pathFavorites" :key="item.id" class="item-row">
            <div class="item-main">
              <strong>{{ item.name }}</strong>
              <span class="value">{{ item.path }}</span>
              <span v-if="item.note" class="note">{{ item.note }}</span>
            </div>
            <div class="item-actions">
              <el-button size="small" @click="openFolder(item.path)">打开</el-button>
              <el-button size="small" @click="editPath(item)">编辑</el-button>
              <el-button size="small" type="danger" plain @click="deletePath(item)">删除</el-button>
            </div>
          </div>
        </div>
      </el-tab-pane>

      <el-tab-pane label="网址" name="url">
        <div class="form-card">
          <h3>{{ editingUrlId ? '编辑网址' : '添加网址' }}</h3>
          <el-input v-model="urlForm.name" placeholder="名称（可选）" class="field-gap" />
          <FavoriteUrlInput v-model="urlForm.url" :show-save="false" placeholder="https://www.yuque.com/..." class="field-gap" />
          <el-input v-model="urlForm.note" placeholder="备注（可选）" class="field-gap" />
          <div class="form-actions">
            <el-button type="primary" @click="submitUrl">{{ editingUrlId ? '保存修改' : '添加' }}</el-button>
            <el-button v-if="editingUrlId" @click="editingUrlId = null; urlForm = { name: '', url: '', note: '' }">取消</el-button>
          </div>
        </div>
        <div v-if="!urlFavorites.length" class="empty-inline">暂无网址收藏</div>
        <div v-else class="item-list">
          <div v-for="item in urlFavorites" :key="item.id" class="item-row">
            <div class="item-main">
              <strong>{{ item.name }}</strong>
              <span class="value">{{ item.url }}</span>
              <span v-if="item.note" class="note">{{ item.note }}</span>
            </div>
            <div class="item-actions">
              <el-button size="small" @click="editUrl(item)">编辑</el-button>
              <el-button size="small" type="danger" plain @click="deleteUrl(item)">删除</el-button>
            </div>
          </div>
        </div>
      </el-tab-pane>

      <el-tab-pane label="对比组合" name="compare">
        <p class="tab-hint">用于「文件夹对比」页的多文件夹组合，与单条地址收藏不同。</p>
        <el-button type="primary" plain size="small" @click="router.push('/compare')">去对比页保存组合</el-button>
        <div v-if="!compareFavorites.length" class="empty-inline">暂无对比组合</div>
        <div v-else class="item-list compare-list">
          <div v-for="fav in compareFavorites" :key="fav.id" class="item-row compare-row">
            <div class="item-main">
              <strong>{{ fav.name }}</strong>
              <el-tag size="small">{{ compareModeLabel(fav.compareMode) }}</el-tag>
              <ul class="folder-mini">
                <li v-for="(f, i) in fav.folders" :key="i">{{ f.label }}：{{ f.path || '未设置' }}</li>
              </ul>
            </div>
            <div class="item-actions">
              <el-button size="small" type="primary" @click="onApplyCompare(fav, true)">加载并对比</el-button>
              <el-button size="small" @click="onApplyCompare(fav, false)">仅加载</el-button>
              <el-button size="small" type="danger" plain @click="removeCompareFavorite(fav.id)">删除</el-button>
            </div>
          </div>
        </div>
      </el-tab-pane>
    </el-tabs>
  </div>
</template>

<style scoped lang="scss">
.favorites-page { padding: 24px; overflow-y: auto; height: calc(100vh - 56px); }
.page-header { margin-bottom: 16px; h2 { margin: 0 0 6px; } p { margin: 0; color: var(--text-muted); font-size: 14px; } }
.page-header-row { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; }
.fav-tabs { margin-top: 8px; }
.form-card {
  padding: 14px 16px; border: 1px solid var(--border); border-radius: 10px;
  background: var(--surface); margin-bottom: 16px;
  h3 { margin: 0 0 12px; font-size: 14px; }
}
.form-grid { display: grid; gap: 10px; }
.field-gap { margin-top: 10px; }
.form-actions { display: flex; gap: 8px; margin-top: 12px; flex-wrap: wrap; }
.empty-inline { color: var(--text-muted); font-size: 13px; padding: 20px 0; text-align: center; }
.tab-hint { font-size: 13px; color: var(--text-muted); margin: 0 0 12px; }
.item-list { display: flex; flex-direction: column; gap: 10px; }
.item-row {
  display: flex; align-items: flex-start; justify-content: space-between; gap: 12px;
  padding: 12px 14px; border: 1px solid var(--border); border-radius: 10px; background: var(--surface);
}
.item-main {
  flex: 1; min-width: 0; font-size: 13px;
  strong { display: block; margin-bottom: 4px; }
  .value { color: var(--text-muted); word-break: break-all; font-size: 12px; }
  .note { display: block; margin-top: 4px; font-size: 11px; color: var(--text-muted); }
}
.item-actions { display: flex; flex-wrap: wrap; gap: 6px; flex-shrink: 0; }
.folder-mini { margin: 6px 0 0; padding-left: 16px; font-size: 11px; color: var(--text-muted); }
.compare-row { flex-direction: column; @media (min-width: 720px) { flex-direction: row; } }
</style>
