<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { usePathUrlFavorites } from '@/composables/usePathUrlFavorites';

const model = defineModel<string>({ default: '' });

withDefaults(
  defineProps<{
    placeholder?: string;
    rows?: number;
    showSave?: boolean;
  }>(),
  {
    placeholder: 'https://...',
    rows: 3,
    showSave: true,
  },
);

const { filterUrlFavorites, promptSaveUrl, reload, findUrlFavorite, urlFavorites } = usePathUrlFavorites();
const open = ref(false);

onMounted(() => reload());

const suggestions = computed(() => filterUrlFavorites(model.value));

const isFavorited = computed(() => {
  urlFavorites.value;
  return !!findUrlFavorite(model.value);
});

function onFocus() {
  open.value = true;
}

function onBlur() {
  window.setTimeout(() => {
    open.value = false;
  }, 160);
}

function selectUrl(url: string) {
  model.value = url;
  open.value = false;
}

async function onSave() {
  await promptSaveUrl(model.value);
}
</script>

<template>
  <div class="favorite-field">
    <div class="input-wrap">
      <el-input
        v-model="model"
        type="textarea"
        :rows="rows"
        :placeholder="placeholder"
        @focus="onFocus"
        @blur="onBlur"
        @input="open = true"
      />
      <ul v-if="open && suggestions.length" class="suggestions">
        <li v-for="item in suggestions" :key="item.id" @mousedown.prevent="selectUrl(item.url)">
          <strong>{{ item.name }}</strong>
          <span class="url">{{ item.url }}</span>
        </li>
      </ul>
    </div>
    <el-button
      v-if="showSave && model.trim()"
      link
      type="warning"
      size="small"
      :class="{ 'is-favorited': isFavorited }"
      @click="onSave"
    >
      <el-icon><StarFilled v-if="isFavorited" /><Star v-else /></el-icon>
      {{ isFavorited ? '已收藏' : '收藏此网址' }}
    </el-button>
  </div>
</template>

<style scoped lang="scss">
.favorite-field {
  width: 100%;
}

.input-wrap {
  position: relative;
  width: 100%;
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

    .url {
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
