import { createRouter, createWebHashHistory } from 'vue-router';

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: '/', redirect: '/compare' },
    { path: '/compare', name: 'compare', component: () => import('@/views/CompareView.vue'), meta: { title: '文件夹对比' } },
    { path: '/rename', name: 'rename', component: () => import('@/views/RenameView.vue'), meta: { title: '批量重命名' } },
    { path: '/favorites', name: 'favorites', component: () => import('@/views/FavoritesView.vue'), meta: { title: '收藏管理' } },
    { path: '/duplicates', name: 'duplicates', component: () => import('@/views/DuplicatesView.vue'), meta: { title: '重复文件' } },
    { path: '/yuque', name: 'yuque', component: () => import('@/views/YuqueView.vue'), meta: { title: '语雀导出' } },
    { path: '/confluence', name: 'confluence', component: () => import('@/views/ConfluenceView.vue'), meta: { title: 'Confluence 转换' } },
    { path: '/settings', name: 'settings', component: () => import('@/views/SettingsView.vue'), meta: { title: '设置' } },
  ],
});

export default router;
