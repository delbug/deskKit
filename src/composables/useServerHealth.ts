import { ref, onMounted } from 'vue';
import { getHealth } from '@/api';

export function useServerHealth() {
  const serverReady = ref(true);
  const serverHint = ref('');

  onMounted(async () => {
    try {
      await getHealth();
      serverReady.value = true;
    } catch {
      serverReady.value = false;
      serverHint.value = 'DeskKit 后端未就绪，请重启应用。';
    }
  });

  return { serverReady, serverHint };
}
