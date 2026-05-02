<script setup lang="ts">
import { ref, watch, onMounted, onBeforeUnmount, computed } from 'vue';
import hljs from 'highlight.js';
import 'highlight.js/styles/github-dark.css';
import { api, ApiError } from '@/api/client';

const props = defineProps<{
  path: string;
  name: string;
}>();

const content = ref('');
const isLoading = ref(false);
const error = ref<string | null>(null);
let inflight: AbortController | null = null;

const highlightedCode = computed(() => {
  if (!content.value) return '';
  const extension = props.name.split('.').pop()?.toLowerCase() || '';

  const langMap: Record<string, string> = {
    'rs': 'rust',
    'ts': 'typescript',
    'js': 'javascript',
    'vue': 'xml',
    'json': 'json',
    'md': 'markdown',
    'css': 'css',
    'scss': 'scss',
    'html': 'xml',
    'toml': 'ini',
    'yml': 'yaml',
    'yaml': 'yaml'
  };

  const lang = langMap[extension];
  try {
    if (lang) {
      return hljs.highlight(content.value, { language: lang }).value;
    }
    return hljs.highlightAuto(content.value).value;
  } catch (e) {
    return content.value;
  }
});

async function loadFile() {
  if (!props.path) return;
  inflight?.abort();
  inflight = new AbortController();
  const ctrl = inflight;

  isLoading.value = true;
  error.value = null;
  try {
    const text = await api.get<string>(
      `/api/fs/read?path=${encodeURIComponent(props.path)}`,
      { signal: ctrl.signal },
    );
    if (ctrl.signal.aborted) return;
    content.value = text;
  } catch (err) {
    if ((err as Error).name === 'AbortError') return;
    error.value = err instanceof ApiError ? err.message : 'Network error while loading file';
  } finally {
    if (!ctrl.signal.aborted) isLoading.value = false;
  }
}

onMounted(loadFile);
watch(() => props.path, loadFile);
onBeforeUnmount(() => inflight?.abort());
</script>

<template>
  <div class="flex flex-col h-full bg-bg-primary overflow-hidden">

    <div class="h-9 flex items-center px-4 bg-bg-sidebar border-b border-border-primary gap-2">
      <svg class="w-3.5 h-3.5 text-text-tertiary" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
      </svg>
      <span class="text-[11px] text-text-tertiary font-mono truncate opacity-60">{{ path }}</span>
    </div>

    <div class="flex-1 overflow-auto relative custom-scrollbar">

      <div v-if="isLoading" class="absolute inset-0 flex items-center justify-center bg-bg-primary/50 backdrop-blur-sm z-10">
        <div class="flex flex-col items-center gap-3">
          <div class="w-6 h-6 border-2 border-accent-blue border-t-transparent rounded-full animate-spin" />
          <span class="text-[10px] text-text-tertiary uppercase tracking-widest font-bold">{{ $t('fileEditor.reading') }}</span>
        </div>
      </div>

      <div v-else-if="error" class="p-12 flex flex-col items-center justify-center text-center gap-4">
        <div class="w-12 h-12 rounded-full bg-accent-red/10 flex items-center justify-center text-accent-red">
          <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" /></svg>
        </div>
        <div>
          <h3 class="text-white text-sm font-bold uppercase tracking-widest mb-1">{{ $t('fileEditor.accessDenied') }}</h3>
          <p class="text-text-tertiary text-[11px]">{{ error }}</p>
        </div>
      </div>

      <pre v-else class="p-6 font-mono text-[13px] leading-relaxed m-0"><code class="hljs" v-html="highlightedCode"></code></pre>
    </div>
  </div>
</template>

<style scoped>
.custom-scrollbar::-webkit-scrollbar {
  width: 10px;
  height: 10px;
}
.custom-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}
.custom-scrollbar::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.05);
  border: 3px solid var(--color-bg-primary);
  border-radius: 10px;
}
.custom-scrollbar::-webkit-scrollbar-thumb:hover {
  background: rgba(255, 255, 255, 0.1);
}

:deep(.hljs) {
  background: transparent !important;
  padding: 0 !important;
}
</style>
