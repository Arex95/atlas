<script setup lang="ts">
import { ref, nextTick } from 'vue';
import { useWorkspaceStore } from '@/stores/workspace';
import { useToast } from '@/composables/useToast';
import NotificationPanel from './NotificationPanel.vue';
import { SERVER_URL, api } from '@/api/client';

defineProps<{
  version: string;
}>();

const store = useWorkspaceStore();
const toast = useToast();
const showNotifications = ref(false);
const notifBtnRef = ref<HTMLElement | null>(null);
const notifStyle = ref<Record<string, string>>({});
const importInputRef = ref<HTMLInputElement | null>(null);
const importing = ref(false);

async function toggleNotifications() {
  showNotifications.value = !showNotifications.value;
  if (showNotifications.value) {
    store.clearUnreadCount();
    await nextTick();
    const rect = notifBtnRef.value?.getBoundingClientRect();
    if (rect) {
      notifStyle.value = {
        position: 'fixed',
        top: `${rect.bottom + 8}px`,
        right: `${window.innerWidth - rect.right}px`,
        zIndex: '9999',
      };
    }
  }
}

function exportDb() {
  const a = document.createElement('a');
  a.href = `${SERVER_URL}/api/db/export`;
  a.download = 'atlas.db';
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
}

function triggerImport() {
  importInputRef.value?.click();
}

async function handleImport(event: Event) {
  const file = (event.target as HTMLInputElement).files?.[0];
  if (!file) return;
  if (!file.name.endsWith('.db') && !file.name.endsWith('.sqlite') && !file.name.endsWith('.sqlite3')) {
    toast.show('Please select a valid SQLite database file (.db)', 'error');
    return;
  }

  importing.value = true;
  try {
    const form = new FormData();
    form.append('file', file);
    const msg = await api.postForm<string>('/api/db/import', form);
    toast.show(msg ?? 'Database imported — restart the server to apply', 'warning', 'Import successful');
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : 'Import failed';
    toast.show(msg, 'error', 'Import error');
  } finally {
    importing.value = false;
    if (importInputRef.value) importInputRef.value.value = '';
  }
}
</script>

<template>
  <div class="flex-none h-9 bg-bg-sidebar border-b border-border-primary flex items-center px-4 justify-between select-none">
    <div class="flex items-center gap-4">
<div class="flex items-center gap-2">
        <span class="font-bold text-[11px] tracking-widest text-text-secondary">ATLAS</span>
        <span class="text-[9px] text-text-tertiary font-bold px-1 border border-border-primary">{{ version }}</span>
      </div>
      <button
        class="flex items-center gap-1.5 px-2 py-1 rounded transition-colors text-text-tertiary hover:text-text-primary hover:bg-white/5"
        :title="$t('topbar.home')"
        @click="store.goHome()"
      >
        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
        </svg>
        <span class="text-[9px] font-black uppercase tracking-wider">Stats</span>
      </button>
    </div>

    <div class="flex-1 max-w-lg mx-auto">
      <div class="flex items-center justify-center gap-2 text-[11px] text-text-tertiary cursor-pointer hover:text-text-secondary transition-all">
        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" /></svg>
        <span>{{ $t('topbar.searchPlaceholder') }}</span>
      </div>
    </div>

    <div class="flex items-center gap-1">
      <button
        class="p-1.5 rounded hover:bg-white/5 transition-colors text-text-tertiary hover:text-text-primary"
        :title="$t('topbar.exportDb')"
        @click="exportDb"
      >
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
        </svg>
      </button>

      <button
        class="p-1.5 rounded hover:bg-white/5 transition-colors"
        :class="importing ? 'text-accent-blue animate-pulse' : 'text-text-tertiary hover:text-text-primary'"
        :disabled="importing"
        :title="$t('topbar.importDb')"
        @click="triggerImport"
      >
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4 4l4-4m0 0l4 4m-4-4V4" />
        </svg>
      </button>
      <input ref="importInputRef" type="file" accept=".db,.sqlite,.sqlite3" class="hidden" @change="handleImport" />

      <div class="w-px h-4 bg-border-primary mx-1" />

      <button
        ref="notifBtnRef"
        class="relative p-1.5 rounded hover:bg-white/5 transition-colors text-text-tertiary hover:text-text-primary"
        :title="$t('topbar.notifications')"
        @click="toggleNotifications"
      >
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M15 17h5l-1.405-1.405A2.032 2.032 0 0118 14.158V11a6.002 6.002 0 00-4-5.659V5a2 2 0 10-4 0v.341C7.67 6.165 6 8.388 6 11v3.159c0 .538-.214 1.055-.595 1.436L4 17h5m6 0v1a3 3 0 11-6 0v-1m6 0H9" />
        </svg>
        <span
          v-if="store.unreadNotificationCount > 0"
          class="absolute -top-0.5 -right-0.5 min-w-[14px] h-[14px] rounded-full bg-accent-blue flex items-center justify-center text-[8px] font-black text-bg-primary px-0.5"
        >
          {{ store.unreadNotificationCount > 99 ? '99+' : store.unreadNotificationCount }}
        </span>
      </button>

      <div class="w-5 h-5 rounded bg-bg-disabled flex items-center justify-center ml-2">
        <span class="text-[9px] text-text-secondary font-bold">A</span>
      </div>
    </div>
  </div>

  <Teleport to="body">
    <div v-if="showNotifications" class="fixed inset-0 z-[9998]" @click="showNotifications = false">
      <div :style="notifStyle" @click.stop>
        <NotificationPanel :project-id="store.selectedProject?.id" />
      </div>
    </div>
  </Teleport>
</template>
