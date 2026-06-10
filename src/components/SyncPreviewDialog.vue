<script setup lang="ts">
import { computed } from 'vue';
import type { SyncPreviewOperation, SyncPreviewSummary } from '@/types';

const props = defineProps<{
  visible: boolean;
  operations: SyncPreviewOperation[];
  summary: SyncPreviewSummary | null;
  loading?: boolean;
}>();

const emit = defineEmits<{
  'update:visible': [value: boolean];
  confirm: [];
}>();

const displayOps = computed(() => props.operations.filter((o) => o.action !== 'skip').slice(0, 200));
const hasMore = computed(() => props.operations.filter((o) => o.action !== 'skip').length > 200);

function actionLabel(action: string) {
  const map: Record<string, string> = {
    copy: '复制',
    overwrite: '覆盖',
    delete: '删除',
    skip: '跳过',
  };
  return map[action] || action;
}

function actionType(action: string) {
  if (action === 'copy') return 'success';
  if (action === 'overwrite') return 'warning';
  if (action === 'delete') return 'danger';
  return 'info';
}

function close() {
  emit('update:visible', false);
}

function confirm() {
  emit('confirm');
}
</script>

<template>
  <el-dialog
    :model-value="visible"
    title="同步预览（Dry-run）"
    width="720px"
    destroy-on-close
    @update:model-value="emit('update:visible', $event)"
  >
    <div v-if="summary" class="preview-summary">
      <el-tag type="success">复制 {{ summary.copy }}</el-tag>
      <el-tag type="warning">覆盖 {{ summary.overwrite }}</el-tag>
      <el-tag type="danger">删除 {{ summary.delete }}</el-tag>
      <el-tag type="info">共 {{ summary.total }} 项操作</el-tag>
    </div>

    <el-table :data="displayOps" max-height="360" size="small" v-loading="loading" empty-text="无需同步，已全部一致">
      <el-table-column label="操作" width="72">
        <template #default="{ row }">
          <el-tag size="small" :type="actionType(row.action)">{{ actionLabel(row.action) }}</el-tag>
        </template>
      </el-table-column>
      <el-table-column label="路径" min-width="220" show-overflow-tooltip prop="relativePath" />
      <el-table-column label="目标" width="100" prop="targetLabel" />
      <el-table-column label="说明" min-width="160" prop="detail" show-overflow-tooltip />
    </el-table>

    <p v-if="hasMore" class="preview-more">仅显示前 200 项，其余 {{ summary!.total - 200 }} 项未列出</p>

    <template #footer>
      <el-button @click="close">取消</el-button>
      <el-button type="primary" :disabled="!summary?.total" :loading="loading" @click="confirm">
        确认执行同步
      </el-button>
    </template>
  </el-dialog>
</template>

<style scoped lang="scss">
.preview-summary {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-bottom: 14px;
}
.preview-more {
  margin: 10px 0 0;
  font-size: 12px;
  color: var(--text-muted);
}
</style>
