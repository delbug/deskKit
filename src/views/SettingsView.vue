<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { ElMessage } from 'element-plus';
import { useConfig } from '@/composables/useConfig';
import ClearCacheButton from '@/components/ClearCacheButton.vue';

const { config, storageLabel, load, persist } = useConfig();
const ignoreText = ref('');
const compareMode = ref('md5');

const settings = computed(() => config.value?.settings);

onMounted(async () => {
  await load();
  if (config.value) {
    ignoreText.value = config.value.settings.ignorePatterns.join('\n');
    compareMode.value = config.value.settings.compareMode;
  }
});

async function saveSettings() {
  try {
    await persist({
      settings: {
        compareMode: compareMode.value as 'md5' | 'name',
        ignorePatterns: ignoreText.value.split(/\n+/).map((s) => s.trim()).filter(Boolean),
        defaultSyncStrategy: settings.value?.defaultSyncStrategy || 'primary-overwrite',
      },
    });
    ElMessage.success('设置已保存');
  } catch (err: any) {
    ElMessage.error(err.message);
  }
}

async function handleClearSettings() {
  await load();
  if (config.value) {
    ignoreText.value = config.value.settings.ignorePatterns.join('\n');
    compareMode.value = config.value.settings.compareMode;
  }
}
</script>

<template>
  <div class="page settings-page">
    <div class="settings-header">
      <h2>设置</h2>
      <ClearCacheButton module="settings" @cleared="handleClearSettings" />
    </div>
    <div class="settings-card">
      <h3>对比默认值</h3>
      <div class="field">
        <label>默认对比模式</label>
        <el-radio-group v-model="compareMode">
          <el-radio value="md5">MD5 内容</el-radio>
          <el-radio value="name">仅路径</el-radio>
        </el-radio-group>
      </div>
      <div class="field">
        <label>忽略项（每行一个，匹配文件名包含即忽略）</label>
        <el-input v-model="ignoreText" type="textarea" :rows="6" placeholder=".DS_Store&#10;node_modules" />
      </div>
      <el-button type="primary" @click="saveSettings">保存设置</el-button>
    </div>
    <div class="settings-card">
      <h3>数据存储</h3>
      <p class="muted">收藏、最近路径、上次对比会话、语雀导出进度等均保存在：</p>
      <code>{{ storageLabel }}（deskit-app-config / deskit-yuque-progress 等键）</code>
      <p class="muted" style="margin-top:12px">刷新页面后会自动从浏览器恢复。服务端不写入任何 JSON 配置文件。</p>
    </div>
    <div class="settings-card">
      <h3>功能说明</h3>
      <ul class="feature-list">
        <li><strong>文件夹对比</strong>：多文件夹 MD5/路径对比，主辅同步、并集、删除、移动</li>
        <li><strong>批量重命名</strong>：前缀/后缀/替换/按位置删除/序号/指定位置插入/清理非法字符</li>
        <li><strong>收藏管理</strong>：保存常用文件夹组合</li>
        <li><strong>重复文件</strong>：按 MD5 查找同一文件夹内的重复项</li>
        <li><strong>语雀导出</strong>：从语雀批量导出 Markdown 文档</li>
        <li><strong>Confluence 转换</strong>：将 Markdown 转为 Confluence 可粘贴的 HTML</li>
      </ul>
    </div>
    <div class="settings-card contact-card">
      <h3>联系维护者</h3>
      <p class="muted">使用中遇到 bug 或有问题，欢迎联系：<a class="contact-email" href="mailto:okwujiang@gmail.com">okwujiang@gmail.com</a></p>
      
    </div>
  </div>
</template>

<style scoped lang="scss">
.settings-page { padding: 24px; overflow-y: auto; height: calc(100vh - 56px); max-width: 720px; }
.settings-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 16px;
  h2 { margin: 0; }
}
.settings-card {
  background: var(--surface); border: 1px solid var(--border); border-radius: 12px;
  padding: 20px; margin-bottom: 16px;
  h3 { margin: 0 0 14px; font-size: 15px; }
}
.field { margin-bottom: 16px; label { display: block; font-size: 13px; color: var(--text-muted); margin-bottom: 6px; } }
.muted { font-size: 13px; color: var(--text-muted); }
code { display: block; padding: 10px; background: var(--bg); border-radius: 6px; font-size: 12px; word-break: break-all; }
.feature-list { margin: 0; padding-left: 20px; line-height: 1.8; font-size: 14px; }
.contact-card {
  margin-bottom: 32px;
  .contact-name {
    margin: 8px 0 4px;
    font-size: 15px;
    font-weight: 600;
  }
  .contact-email {
    font-size: 14px;
    color: #93c5fd;
    text-decoration: none;
    &:hover { text-decoration: underline; }
  }
}
</style>
