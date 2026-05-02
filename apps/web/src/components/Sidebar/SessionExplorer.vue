<script setup lang="ts">
import { ref, onMounted, watch } from 'vue';

import FileRow, { type FileNode } from './FileRow.vue';
import { api } from '@/api/client';

const props = defineProps<{
  rootPath: string;
}>();

const emit = defineEmits<{
  'file-selected': [path: string]
  'cd-requested': [path: string]
}>();

const files = ref<FileNode[]>([]);
const isLoading = ref(false);
const collapsed = ref(false);

async function fetchFiles(path: string): Promise<FileNode[]> {
  if (!path) return [];
  try {
    return await api.get<FileNode[]>(`/api/fs/list?path=${encodeURIComponent(path)}`);
  } catch {
    return [];
  }
}

async function loadRoot() {
  if (!props.rootPath) return;
  isLoading.value = true;
  files.value = await fetchFiles(props.rootPath);
  isLoading.value = false;
}

async function handleAction(item: FileNode) {
  if (item.is_dir) {
    if (item.isOpen) {
      item.isOpen = false;
    } else {
      item.isOpen = true;
      if (!item.children || item.children.length === 0) {
        item.isLoading = true;
        item.children = await fetchFiles(item.path);
        item.isLoading = false;
      }
    }
  } else {
    emit('file-selected', item.path);
  }
}

onMounted(loadRoot);
watch(() => props.rootPath, loadRoot);
</script>

<template>
  <!-- collapsed strip -->
  <div v-if="collapsed" class="flex flex-col w-8 flex-none h-full bg-bg-sidebar/30 border-r border-white/5 items-center py-2 gap-3">
    <button
      class="p-1.5 rounded hover:bg-white/10 text-text-tertiary hover:text-white transition-colors"
      title="Expand context"
      @click="collapsed = false"
    >
      <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
        <path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" />
      </svg>
    </button>
    <span class="text-[8px] font-black uppercase tracking-[0.2em] text-text-tertiary opacity-50 [writing-mode:vertical-lr] rotate-180 select-none">
      {{ $t('sessionExplorer.title') }}
    </span>
  </div>

  <!-- expanded panel -->
  <div v-else class="flex flex-col w-64 flex-none h-full bg-bg-sidebar/30 border-r border-white/5 overflow-hidden">
    <div class="px-3 py-2 border-b border-white/5 flex items-center justify-between bg-black/20">
      <div class="flex items-center gap-2">
        <svg class="w-3 h-3 text-accent-blue" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
          <path d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
        </svg>
        <span class="text-[9px] font-black uppercase tracking-[0.2em] text-text-tertiary">{{ $t('sessionExplorer.title') }}</span>
      </div>
      <div class="flex items-center gap-1">
        <button @click="loadRoot" class="p-1 hover:bg-white/5 rounded transition-colors text-text-tertiary hover:text-white">
          <svg class="w-3 h-3 opacity-60" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"/>
          </svg>
        </button>
        <button
          class="p-1 hover:bg-white/5 rounded transition-colors text-text-tertiary hover:text-white"
          title="Collapse context"
          @click="collapsed = true"
        >
          <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M15 19l-7-7 7-7" />
          </svg>
        </button>
      </div>
    </div>

    <div class="flex-1 overflow-y-auto scrollbar-hide py-2">
      <div v-if="isLoading && files.length === 0" class="flex items-center justify-center p-8">
        <div class="w-4 h-4 border border-accent-blue border-t-transparent rounded-full animate-spin" />
      </div>
      <div v-else class="space-y-px">
        <template v-for="item in files" :key="item.path">
          <FileRow
            :item="item"
            :depth="0"
            @action="handleAction"
            @cd="emit('cd-requested', $event)"
          />
        </template>
      </div>
    </div>
  </div>
</template>

<style scoped>
.scrollbar-hide::-webkit-scrollbar { display: none; }
</style>
