<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { pickFolder } from '@/api';
import { usePathUrlFavorites } from '@/composables/usePathUrlFavorites';

const model = defineModel<string>({ default: '' });

withDefaults(
  defineProps<{
    placeholder?: string;
    readonly?: boolean;
    showPicker?: boolean;
    showSave?: boolean;
    size?: 'default' | 'small' | 'large';
  }>(),
  {
    placeholder: '文件夹路径',
    readonly: false,
    showPicker: true,
    showSave: true,
    size: 'default',
  },
);

const { filterPathFavorites, promptSavePath, reload, findPathFavorite, pathFavorites } = usePathUrlFavorites();
const open = ref(false);

onMounted(() => reload());

const suggestions = computed(() => filterPathFavorites(model.value));

const isFavorited = computed(() => {
  pathFavorites.value;
  return !!findPathFavorite(model.value);
});

function onFocus() {
  open.value = true;
}

function onBlur() {
  window.setTimeout(() => {
    open.value = false;
  }, 160);
}

function selectPath(path: string) {
  model.value = path;
  open.value = false;
}

async function onPick() {
  const res = await pickFolder();
  if (!res.cancelled) {
    model.value = res.path;
    open.value = false;
  }
}

async function onSave() {
  await promptSavePath(model.value);
}
</script>

<template>
  <div class="favorite-field">
    <div class="input-row">
      <div class="input-wrap">
        <el-input
          v-model="model"
          :placeholder="placeholder"
          :readonly="readonly"
          :size="size"
          clearable
          @focus="onFocus"
          @blur="onBlur"
          @input="open = true"
        />
        <ul v-if="open && suggestions.length" class="suggestions">
          <li v-for="item in suggestions" :key="item.id" @mousedown.prevent="selectPath(item.path)">
            <strong>{{ item.name }}</strong>
            <span class="path">{{ item.path }}</span>
          </li>
        </ul>
      </div>
      <el-button v-if="showPicker" :size="size" @click="onPick">
        <el-icon><Folder /></el-icon>
      </el-button>
      <el-button
        v-if="showSave && model.trim()"
        :size="size"
        link
        type="warning"
        :class="{ 'is-favorited': isFavorited }"
        :title="isFavorited ? '已在收藏中' : '收藏此路径'"
        @click="onSave"
      >
        <el-icon><StarFilled v-if="isFavorited" /><Star v-else /></el-icon>
      </el-button>
    </div>
  </div>
</template>

<style scoped lang="scss">
.favorite-field {
  width: 100%;
}

.input-row {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  width: 100%;
}

.input-wrap {
  position: relative;
  flex: 1;
  min-width: 0;
}

.suggestions {
  position: absolute;
  z-index: 20;
  left: 0;
  right: 0;
  top: calc(100% + 4px);
  margin: 0;
  padding: 4px 0;
  list-style: none;
  max-height: 240px;
  overflow-y: auto;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 8px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.25);

  li {
    padding: 8px 12px;
    cursor: pointer;
    font-size: 12px;

    &:hover {
      background: var(--surface-2);
    }

    strong {
      display: block;
      margin-bottom: 2px;
    }

    .path {
      color: var(--text-muted);
      word-break: break-all;
    }
  }
}

.is-favorited {
  :deep(.el-icon) {
    color: #f59e0b;
  }
}
</style>
