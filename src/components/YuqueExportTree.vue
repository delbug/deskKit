<script setup lang="ts">
import type { DocTreeNode } from '@/utils/yuque-doc-tree';

defineOptions({ name: 'YuqueExportTree' });

defineProps<{
  nodes: DocTreeNode[];
  depth?: number;
}>();

function statusIcon(status?: string) {
  if (status === 'done') return '✓';
  if (status === 'failed') return '✗';
  if (status === 'exporting') return '…';
  return '○';
}
</script>

<template>
  <ul class="tree-list" :class="{ root: (depth ?? 0) === 0 }">
    <li v-for="node in nodes" :key="node.id" class="tree-item">
      <div v-if="node.type === 'folder'" class="tree-folder">
        <span class="folder-icon">📁</span>
        <span class="folder-name">{{ node.name }}</span>
        <span class="folder-stats">{{ node.doneCount }}/{{ node.totalCount }}</span>
      </div>
      <div v-else class="tree-doc" :class="node.doc?.status">
        <span class="status-icon">{{ statusIcon(node.doc?.status) }}</span>
        <span class="doc-title">{{ node.name }}</span>
        <span v-if="node.doc?.status === 'exporting'" class="tag exporting">下载中</span>
        <span v-else-if="node.doc?.status === 'failed'" class="tag failed" :title="node.doc.failMessage">失败</span>
      </div>
      <YuqueExportTree
        v-if="node.children?.length"
        :nodes="node.children"
        :depth="(depth ?? 0) + 1"
      />
    </li>
  </ul>
</template>

<style scoped lang="scss">
.tree-list {
  list-style: none;
  margin: 0;
  padding-left: 16px;

  &.root {
    padding-left: 0;
  }
}

.tree-item {
  margin: 2px 0;
}

.tree-folder {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 6px;
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
  border-radius: 4px;

  .folder-stats {
    margin-left: auto;
    font-size: 11px;
    font-weight: 400;
    color: var(--text-muted);
  }
}

.tree-doc {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 3px 6px;
  font-size: 13px;
  border-radius: 4px;

  &.done {
    color: var(--text-muted);

    .status-icon {
      color: #22c55e;
      font-weight: 700;
    }

    .doc-title {
      text-decoration: line-through;
      opacity: 0.85;
    }
  }

  &.exporting {
    background: rgba(59, 130, 246, 0.12);

    .status-icon {
      color: #3b82f6;
    }
  }

  &.failed {
    .status-icon {
      color: #ef4444;
      font-weight: 700;
    }
  }

  &.pending .status-icon {
    color: var(--text-muted);
    opacity: 0.5;
  }
}

.status-icon {
  width: 16px;
  text-align: center;
  flex-shrink: 0;
  font-size: 12px;
}

.doc-title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tag {
  font-size: 11px;
  padding: 1px 6px;
  border-radius: 4px;
  flex-shrink: 0;

  &.exporting {
    background: rgba(59, 130, 246, 0.2);
    color: #60a5fa;
  }

  &.failed {
    background: rgba(239, 68, 68, 0.15);
    color: #f87171;
  }
}
</style>
