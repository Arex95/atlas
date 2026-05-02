<script setup lang="ts">
import { ref, nextTick } from 'vue';
import McpConfigPanel from '@/components/McpConfigPanel.vue';
import ProfileModal from '@/components/ProfileModal.vue';
import GlobalContext from '@/components/GlobalContext.vue';
import { useProfileStore } from '@/stores/profile';

const profile = useProfileStore();
const showMcpConfig = ref(false);
const showProfile = ref(false);
const showGlobalContext = ref(false);
const btnRef = ref<HTMLElement | null>(null);
const panelStyle = ref<Record<string, string>>({});

async function toggleMcp() {
  showMcpConfig.value = !showMcpConfig.value;
  if (showMcpConfig.value) {
    await nextTick();
    const rect = btnRef.value?.getBoundingClientRect();
    if (rect) {
      panelStyle.value = {
        position: 'fixed',
        bottom: `${window.innerHeight - rect.top + 8}px`,
        left: `${rect.left}px`,
        zIndex: '9999',
      };
    }
  }
}
</script>

<template>
  <div class="px-4 py-3 border-t border-border-primary bg-bg-sidebar">
    <div class="flex items-center gap-3">
      <button
        class="flex items-center gap-3 flex-1 min-w-0 group"
        :title="$t('sidebarFooter.editProfile')"
        @click="showProfile = true"
      >
        <div
          class="w-8 h-8 rounded-lg flex-none flex items-center justify-center text-[11px] font-black text-white border border-white/10 group-hover:border-white/20 transition-colors"
          :style="{ backgroundColor: profile.profile.avatarColor }"
        >
          {{ profile.initials }}
        </div>
        <div class="flex flex-col min-w-0 text-left">
          <span class="text-[12px] font-bold text-text-primary group-hover:text-white truncate transition-colors leading-tight">
            {{ profile.profile.name }}
          </span>
          <span class="text-[9px] text-text-tertiary uppercase font-black tracking-widest truncate opacity-60">
            {{ profile.profile.title }}
          </span>
        </div>
      </button>

      <div class="flex items-center gap-2 flex-none">
        <button
          class="flex items-center gap-1 px-1.5 py-1 rounded hover:bg-white/5 transition-colors"
          :class="showGlobalContext ? 'text-accent-blue' : 'text-text-tertiary'"
          title="Global Context"
          @click="showGlobalContext = true"
        >
          <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 21a9.004 9.004 0 0 0 8.716-6.747M12 21a9.004 9.004 0 0 1-8.716-6.747M12 21c2.485 0 4.5-4.03 4.5-9S14.485 3 12 3m0 18c-2.485 0-4.5-4.03-4.5-9S9.515 3 12 3m0 0a8.997 8.997 0 0 1 7.843 4.582M12 3a8.997 8.997 0 0 0-7.843 4.582m15.686 0A11.953 11.953 0 0 1 12 10.5c-2.998 0-5.74-1.1-7.843-2.918m15.686 0A8.959 8.959 0 0 1 21 12c0 .778-.099 1.533-.284 2.253m0 0A17.919 17.919 0 0 1 12 16.5c-3.162 0-6.133-.815-8.716-2.247m0 0A9.015 9.015 0 0 1 3 12c0-1.605.42-3.113 1.157-4.418" />
          </svg>
        </button>
        <button
          ref="btnRef"
          class="flex items-center gap-1 px-1.5 py-1 rounded hover:bg-white/5 transition-colors"
          :class="showMcpConfig ? 'text-accent-blue' : 'text-text-tertiary'"
          :title="$t('sidebarFooter.mcpConfig')"
          @click="toggleMcp"
        >
          <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
            <path stroke-linecap="round" stroke-linejoin="round" d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4" />
          </svg>
          <span class="text-[8px] font-black uppercase tracking-wider">MCP</span>
        </button>
        <div class="w-2 h-2 rounded-full bg-accent-green" />
      </div>
    </div>
  </div>

  <Teleport to="body">
    <div v-if="showMcpConfig" class="fixed inset-0 z-[9998]" @click="showMcpConfig = false">
      <div :style="panelStyle" @click.stop>
        <McpConfigPanel />
      </div>
    </div>
    <ProfileModal v-if="showProfile" @close="showProfile = false" />
    <GlobalContext v-if="showGlobalContext" @close="showGlobalContext = false" />
  </Teleport>
</template>
