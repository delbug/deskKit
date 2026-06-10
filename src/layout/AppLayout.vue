<script setup lang="ts">
import { computed } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useServerHealth } from '@/composables/useServerHealth';

const route = useRoute();
const router = useRouter();
const { serverReady, serverHint } = useServerHealth();

const menus = [
  { path: '/compare', title: '文件夹对比', icon: 'DocumentCopy' },
  { path: '/rename', title: '批量重命名', icon: 'EditPen' },
  { path: '/duplicates', title: '重复文件', icon: 'CopyDocument' },
  { path: '/find-files', title: '查找文件', icon: 'Search' },
  { path: '/yuque', title: '语雀导出', icon: 'Download' },
  { path: '/confluence', title: 'Confluence 转换', icon: 'Document' },
  { path: '/favorites', title: '收藏管理', icon: 'Star' },
  { path: '/settings', title: '设置', icon: 'Setting' },
];

const active = computed(() => route.path);
</script>

<template>
  <div class="app-shell">
    <header class="app-header">
      <div class="brand" @click="router.push('/compare')">
        <el-icon><FolderOpened /></el-icon>
        <h1>DeskKit</h1>
        <span class="tauri-badge">Tauri</span>
      </div>
      <nav class="top-nav">
        <button
          v-for="m in menus"
          :key="m.path"
          class="nav-item"
          :class="{ active: active === m.path }"
          @click="router.push(m.path)"
        >
          <el-icon><component :is="m.icon" /></el-icon>
          {{ m.title }}
        </button>
      </nav>
    </header>
    <main class="app-main">
      <el-alert
        v-if="!serverReady"
        type="error"
        :closable="false"
        show-icon
        class="server-alert"
        :title="serverHint"
      />
      <router-view />
    </main>
  </div>
</template>

<style scoped lang="scss">
.brand {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  h1 {
    margin: 0;
    font-size: 18px;
    font-weight: 700;
  }
}

.top-nav {
  display: flex;
  gap: 4px;
  margin-left: 24px;
  flex-wrap: wrap;
}

.nav-item {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 8px 14px;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: var(--text-muted);
  font-size: 13px;
  cursor: pointer;
  transition: all 0.15s;

  &:hover {
    background: var(--surface-2);
    color: var(--text);
  }

  &.active {
    background: rgba(59, 130, 246, 0.15);
    color: #93c5fd;
    font-weight: 600;
  }
}

.server-alert {
  margin: 12px 16px 0;
  flex-shrink: 0;
}

.app-main {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.tauri-badge {
  font-size: 10px;
  padding: 2px 6px;
  border-radius: 4px;
  background: rgba(34, 197, 94, 0.15);
  color: #86efac;
  border: 1px solid rgba(34, 197, 94, 0.3);
  margin-left: 4px;
}
</style>
