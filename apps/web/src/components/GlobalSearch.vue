<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted, nextTick } from 'vue';
import { api } from '@/api/client';
import { useWorkspaceStore } from '@/stores/workspace';

const emit = defineEmits<{ close: [] }>();
const store = useWorkspaceStore();

interface SearchResult {
  kind: string;
  id: string;
  title: string;
  subtitle?: string;
  url?: string;
}

const query = ref('');
const results = ref<SearchResult[]>([]);
const selectedIndex = ref(0);
const loading = ref(false);
const inputRef = ref<HTMLInputElement | null>(null);

let debounceTimer: ReturnType<typeof setTimeout> | null = null;

watch(query, (val) => {
  if (debounceTimer) clearTimeout(debounceTimer);
  if (!val.trim()) {
    results.value = [];
    return;
  }
  debounceTimer = setTimeout(async () => {
    loading.value = true;
    try {
      results.value = await api.get<SearchResult[]>(`/api/search?q=${encodeURIComponent(val)}`);
      selectedIndex.value = 0;
    } catch {
      results.value = [];
    } finally {
      loading.value = false;
    }
  }, 200);
});

function kindIcon(kind: string): string {
  if (kind === 'project') return '◈';
  if (kind === 'task') return '◎';
  if (kind === 'session') return '▷';
  if (kind === 'document') return '◻';
  return '·';
}

function kindColor(kind: string): string {
  if (kind === 'project') return 'text-accent-blue';
  if (kind === 'task') return 'text-accent-green';
  if (kind === 'session') return 'text-accent-purple';
  if (kind === 'document') return 'text-text-secondary';
  return 'text-text-tertiary';
}

function select(result: SearchResult) {
  if (result.kind === 'project' && result.url) {
    const slug = result.url.replace('/projects/', '');
    store.selectProject(slug);
  } else if (result.kind === 'session') {
    const session = store.tabs.find(t => t.id === result.id);
    if (session) store.setActiveTab(result.id);
  }
  emit('close');
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'ArrowDown') {
    e.preventDefault();
    selectedIndex.value = Math.min(selectedIndex.value + 1, results.value.length - 1);
  } else if (e.key === 'ArrowUp') {
    e.preventDefault();
    selectedIndex.value = Math.max(selectedIndex.value - 1, 0);
  } else if (e.key === 'Enter' && results.value[selectedIndex.value]) {
    select(results.value[selectedIndex.value]);
  } else if (e.key === 'Escape') {
    emit('close');
  }
}

function onOverlayClick(e: MouseEvent) {
  if ((e.target as HTMLElement).dataset.overlay === 'true') emit('close');
}

onMounted(() => nextTick(() => inputRef.value?.focus()));
onUnmounted(() => { if (debounceTimer) clearTimeout(debounceTimer); });
</script>

<template>
  <Teleport to="body">
    <div
      class="fixed inset-0 z-50 flex items-start justify-center pt-[15vh] bg-black/70 backdrop-blur-sm"
      data-overlay="true"
      @click="onOverlayClick"
    >
      <div class="w-full max-w-xl bg-bg-elevated border border-border-primary rounded-lg shadow-2xl overflow-hidden">
        <!-- Input -->
        <div class="flex items-center gap-3 px-4 py-3 border-b border-border-primary">
          <svg class="w-4 h-4 text-text-tertiary flex-none" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <circle cx="11" cy="11" r="8"/><path d="m21 21-4.35-4.35"/>
          </svg>
          <input
            ref="inputRef"
            v-model="query"
            :placeholder="$t('globalSearch.placeholder')"
            class="flex-1 bg-transparent text-[13px] text-text-primary placeholder-text-tertiary outline-none font-mono"
            @keydown="onKeydown"
          />
          <span v-if="loading" class="text-[10px] text-text-tertiary animate-pulse">{{ $t('globalSearch.searching') }}</span>
          <kbd class="text-[9px] text-text-tertiary border border-border-primary rounded px-1 py-0.5 font-mono">ESC</kbd>
        </div>

        <!-- Results -->
        <div class="max-h-[50vh] overflow-y-auto">
          <template v-if="results.length > 0">
            <button
              v-for="(result, i) in results"
              :key="result.kind + result.id"
              class="w-full flex items-center gap-3 px-4 py-3 text-left transition-colors border-b border-border-primary/30 last:border-0"
              :class="selectedIndex === i ? 'bg-bg-sidebar/60' : 'hover:bg-bg-sidebar/30'"
              @click="select(result)"
              @mouseenter="selectedIndex = i"
            >
              <span class="text-[16px] leading-none flex-none" :class="kindColor(result.kind)">{{ kindIcon(result.kind) }}</span>
              <div class="flex-1 min-w-0">
                <p class="text-[12px] font-bold text-text-primary truncate">{{ result.title }}</p>
                <p v-if="result.subtitle" class="text-[10px] text-text-tertiary font-mono truncate mt-0.5">{{ result.subtitle }}</p>
              </div>
              <span class="text-[9px] font-black uppercase tracking-widest text-text-tertiary/60 flex-none">{{ result.kind }}</span>
            </button>
          </template>

          <div v-else-if="query.trim() && !loading" class="flex items-center justify-center py-10 text-text-tertiary">
            <p class="text-[11px] font-mono">{{ $t('globalSearch.noResults', { query }) }}</p>
          </div>

          <div v-else-if="!query.trim()" class="px-4 py-3">
            <p class="text-[10px] text-text-tertiary font-mono opacity-60">{{ $t('globalSearch.emptyHint') }}</p>
          </div>
        </div>

        <!-- Footer -->
        <div class="px-4 py-2 border-t border-border-primary flex items-center gap-4">
          <div class="flex items-center gap-1.5 text-[9px] text-text-tertiary">
            <kbd class="border border-border-primary rounded px-1 py-0.5 font-mono">↑↓</kbd>
            <span>{{ $t('globalSearch.navigate') }}</span>
          </div>
          <div class="flex items-center gap-1.5 text-[9px] text-text-tertiary">
            <kbd class="border border-border-primary rounded px-1 py-0.5 font-mono">↵</kbd>
            <span>{{ $t('globalSearch.open') }}</span>
          </div>
        </div>
      </div>
    </div>
  </Teleport>
</template>
