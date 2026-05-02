<script setup lang="ts">
import { ref, onMounted, watch } from 'vue';
import type { Project } from '@atlas/domain';
import FileRow, { type FileNode } from './FileRow.vue';
import { api } from '@/api/client';

const props = defineProps<{
  project: Project;
  activePath?: string;
}>();

const emit = defineEmits<{
  'file-selected': [path: string]
  'cd-requested': [path: string]
}>();

const files = ref<FileNode[]>([]);
const isLoading = ref(false);

async function fetchFiles(path: string): Promise<FileNode[]> {
  if (!path) return [];
  try {
    return await api.get<FileNode[]>(`/api/fs/list?path=${encodeURIComponent(path)}`);
  } catch {
    return [];
  }
}

async function loadRoot() {
  const root = props.activePath || props.project?.rootPath;
  if (!root) return;
  isLoading.value = true;
  files.value = await fetchFiles(root);
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
watch(() => props.project?.id, loadRoot);
watch(() => props.activePath, loadRoot);

</script>

<template>
  <div class="flex flex-col h-full bg-bg-sidebar overflow-hidden border-r border-white/5">

    <div class="px-4 py-2 border-b border-white/5 flex items-center justify-between">
      <span class="text-[10px] font-bold uppercase tracking-widest text-text-secondary opacity-50">{{ $t('fileExplorer.title') }}</span>
      <button @click="loadRoot" class="p-1 hover:bg-white/5 rounded transition-colors" :title="$t('fileExplorer.refresh')">
        <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="opacity-50"><path d="M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/></svg>
      </button>
    </div>

    <div v-if="isLoading && files.length === 0" class="p-8 flex flex-col items-center justify-center gap-3 opacity-50">
      <div class="w-5 h-5 border-2 border-accent-blue border-t-transparent rounded-full animate-spin" />
      <span class="text-[9px] uppercase tracking-[0.2em] font-black">{{ $t('fileExplorer.indexing') }}</span>
    </div>

    <div v-else class="flex-1 overflow-y-auto scrollbar-hide py-2">
      <div class="space-y-px">
        <template v-for="item in files" :key="item.path">
          <FileRow :item="item" @action="handleAction" @cd="emit('cd-requested', $event)" />
        </template>
      </div>
    </div>
  </div>
</template>

<style scoped>
.scrollbar-hide::-webkit-scrollbar {
  display: none;
}
.scrollbar-hide {
  -ms-overflow-style: none;
  scrollbar-width: none;
}
</style>
