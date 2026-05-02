<script setup lang="ts">
export interface FileNode {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
  isOpen?: boolean;
  isLoading?: boolean;
  children?: FileNode[];
}

const props = defineProps<{
  item: FileNode;
  depth?: number;
}>();

const emit = defineEmits<{
  action: [item: FileNode]
  cd: [path: string]
}>();

const getFileIcon = (item: FileNode) => {
  if (item.is_dir) {
    return item.isOpen ? '📂' : '📁';
  }
  const ext = item.name.split('.').pop()?.toLowerCase();
  switch (ext) {
    case 'ts':
    case 'tsx': return '🔷';
    case 'js':
    case 'jsx': return '🟨';
    case 'vue': return '💚';
    case 'rs': return '🦀';
    case 'json': return '📦';
    case 'md': return '📝';
    case 'css':
    case 'scss': return '🎨';
    case 'html': return '🌐';
    default: return '📄';
  }
};
</script>

<template>
  <div class="group">
    <div 
      @click="emit('action', item)"
      :style="{ paddingLeft: ((depth || 0) * 12 + 12) + 'px' }"
      class="flex items-center gap-2.5 py-1 px-3 cursor-pointer transition-all duration-150 border-l-2 border-transparent hover:bg-white/[0.03] hover:border-accent-blue/30 group/row"
    >
      <span class="text-[13px] flex-none opacity-60 group-hover/row:opacity-100 transition-opacity">{{ getFileIcon(item) }}</span>
      <span class="text-[11.5px] text-text-secondary group-hover/row:text-white truncate font-medium tracking-tight">
        {{ item.name }}
      </span>

      
      <div v-if="item.is_dir" class="ml-auto flex items-center gap-2 opacity-0 group-hover/row:opacity-100 transition-opacity">
        <button 
          @click.stop="emit('cd', item.path)"
          class="p-1 hover:bg-accent-blue/20 rounded text-accent-blue transition-colors"
          :title="$t('fileRow.openTerminalHere')"
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg>
        </button>
      </div>

      <div v-if="item.isLoading" class="ml-auto w-2.5 h-2.5 border border-accent-blue border-t-transparent rounded-full animate-spin" />
    </div>

    <div v-if="item.isOpen && item.children" class="overflow-hidden">
      <FileRow 
        v-for="child in item.children" 
        :key="child.path" 
        :item="child" 
        :depth="(depth || 0) + 1"
        @action="emit('action', $event)"
        @cd="emit('cd', $event)"
      />
    </div>
  </div>
</template>

<script lang="ts">
export default {
  name: 'FileRow'
}
</script>
