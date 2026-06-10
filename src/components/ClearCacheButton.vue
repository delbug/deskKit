<script setup lang="ts">
import { ElMessage } from 'element-plus';
import { useModuleClear, type ModuleId } from '@/composables/useModuleClear';

const props = defineProps<{
  module: ModuleId;
  saveDir?: string;
  url?: string;
}>();

const emit = defineEmits<{
  cleared: [];
}>();

const { clearing, confirmAndClear } = useModuleClear(props.module);

async function handleClear() {
  const ok = await confirmAndClear(
    async () => {
      emit('cleared');
    },
    props.saveDir || props.url
      ? { saveDir: props.saveDir, url: props.url }
      : undefined,
  );
  if (ok) ElMessage.success('已清除本页历史数据与缓存');
}
</script>

<template>
  <el-button
    type="danger"
    plain
    size="small"
    :loading="clearing"
    @click="handleClear"
  >
    <el-icon><Delete /></el-icon>
    清除历史数据
  </el-button>
</template>

<style scoped lang="scss">
.el-button {
  flex-shrink: 0;
}
</style>
