<script setup lang="ts">
import { nextTick, ref, watch } from 'vue';
import type { DocTreeNode } from '@/utils/yuque-doc-tree';
import { docTreeToElTreeNodes, collectDocSlugsFromTree } from '@/utils/yuque-doc-tree';

defineOptions({ name: 'YuqueExportSelectTree' });

const props = defineProps<{
  nodes: DocTreeNode[];
  modelValue: string[];
}>();

const emit = defineEmits<{
  'update:modelValue': [slugs: string[]];
}>();

const treeRef = ref<{
  setCheckedKeys: (keys: string[], leafOnly?: boolean) => void;
  getCheckedKeys: (leafOnly?: boolean) => string[];
}>();

const treeData = ref<{ id: string; label: string; children?: unknown[] }[]>([]);
let syncing = false;

function docKeysFromSlugs(slugs: string[]) {
  return slugs.map((s) => `doc:${s}`);
}

function slugsFromCheckedKeys(keys: string[]) {
  return keys.filter((k) => k.startsWith('doc:')).map((k) => k.slice(4));
}

function syncCheckedFromModel() {
  if (!treeRef.value || syncing) return;
  syncing = true;
  treeRef.value.setCheckedKeys(docKeysFromSlugs(props.modelValue), false);
  nextTick(() => {
    syncing = false;
  });
}

function onCheck() {
  if (!treeRef.value || syncing) return;
  const keys = treeRef.value.getCheckedKeys(false) as string[];
  emit('update:modelValue', slugsFromCheckedKeys(keys));
}

watch(
  () => props.nodes,
  (nodes) => {
    treeData.value = docTreeToElTreeNodes(nodes);
    nextTick(() => syncCheckedFromModel());
  },
  { immediate: true, deep: true },
);

watch(
  () => props.modelValue,
  () => syncCheckedFromModel(),
  { deep: true },
);

defineExpose({
  selectAll() {
    emit('update:modelValue', collectDocSlugsFromTree(props.nodes));
  },
  clearAll() {
    emit('update:modelValue', []);
  },
});
</script>

<template>
  <el-tree
    ref="treeRef"
    class="yuque-select-tree"
    :data="treeData"
    show-checkbox
    node-key="id"
    default-expand-all
    :props="{ label: 'label', children: 'children' }"
    @check="onCheck"
  />
</template>

<style scoped lang="scss">
.yuque-select-tree {
  font-size: 13px;
  background: transparent;

  :deep(.el-tree-node__content) {
    height: 28px;
  }
}
</style>
