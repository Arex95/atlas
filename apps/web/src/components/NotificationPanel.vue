<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import type { AtlasNotification } from '@atlas/domain';
import { api } from '@/api/client';

const props = defineProps<{ projectId?: string }>();

const notifications = ref<AtlasNotification[]>([]);
const loading = ref(false);

const unreadCount = computed(() => notifications.value.filter((n) => n.status === 'unread').length);

const kindColor: Record<string, string> = {
  info: 'text-accent-blue',
  success: 'text-accent-green',
  warning: 'text-yellow-400',
  error: 'text-red-400',
  reminder: 'text-purple-400',
};

async function load() {
  loading.value = true;
  try {
    const q = props.projectId ? `?projectId=${props.projectId}` : '';
    notifications.value = await api.get<AtlasNotification[]>(`/api/notifications${q}`);
  } finally {
    loading.value = false;
  }
}

async function markAllRead() {
  await api.post('/api/notifications/mark-all-read', {});
  notifications.value = notifications.value.map((n) => ({ ...n, status: 'read' }));
}

async function remove(id: string) {
  await api.delete(`/api/notifications/${id}`);
  notifications.value = notifications.value.filter((n) => n.id !== id);
}

function formatTime(ts: string) {
  try {
    const d = new Date(ts);
    return d.toLocaleString([], { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
  } catch {
    return ts;
  }
}

onMounted(load);
</script>

<template>
  <div class="w-80 bg-bg-elevated border border-border-primary rounded-lg shadow-2xl overflow-hidden animate-in fade-in slide-in-from-top-2 duration-200">
    <div class="h-10 px-4 flex items-center justify-between border-b border-border-primary bg-bg-sidebar">
      <div class="flex items-center gap-2">
        <svg class="w-3.5 h-3.5 text-accent-blue" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 17h5l-1.405-1.405A2.032 2.032 0 0118 14.158V11a6.002 6.002 0 00-4-5.659V5a2 2 0 10-4 0v.341C7.67 6.165 6 8.388 6 11v3.159c0 .538-.214 1.055-.595 1.436L4 17h5m6 0v1a3 3 0 11-6 0v-1m6 0H9" />
        </svg>
        <span class="text-[10px] font-black uppercase tracking-wider text-text-secondary">
          {{ $t('notifications.title') }}
        </span>
        <span v-if="unreadCount > 0" class="text-[9px] px-1.5 py-0.5 rounded-full bg-accent-blue text-bg-primary font-black">
          {{ unreadCount }}
        </span>
      </div>
      <div class="flex items-center gap-2">
        <button
          v-if="unreadCount > 0"
          class="text-[9px] text-text-tertiary hover:text-accent-blue transition-colors uppercase tracking-wider"
          @click="markAllRead"
        >
          {{ $t('notifications.markAllRead') }}
        </button>
      </div>
    </div>

    <div class="max-h-80 overflow-y-auto scrollbar-hide">
      <div v-if="loading" class="flex items-center justify-center p-8 opacity-40">
        <div class="w-4 h-4 border border-accent-blue border-t-transparent rounded-full animate-spin" />
      </div>

      <div v-else-if="notifications.length === 0" class="flex flex-col items-center justify-center p-8 text-center opacity-40">
        <svg class="w-8 h-8 text-text-tertiary mb-2" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M15 17h5l-1.405-1.405A2.032 2.032 0 0118 14.158V11a6.002 6.002 0 00-4-5.659V5a2 2 0 10-4 0v.341C7.67 6.165 6 8.388 6 11v3.159c0 .538-.214 1.055-.595 1.436L4 17h5m6 0v1a3 3 0 11-6 0v-1m6 0H9" />
        </svg>
        <p class="text-[10px] text-text-tertiary">{{ $t('notifications.empty') }}</p>
      </div>

      <div v-else class="divide-y divide-border-primary">
        <div
          v-for="n in notifications"
          :key="n.id"
          class="flex items-start gap-3 px-4 py-3 hover:bg-white/[0.02] transition-colors group"
          :class="n.status === 'unread' ? 'bg-white/[0.01]' : ''"
        >
          <div class="flex-none w-1.5 h-1.5 rounded-full mt-1.5" :class="kindColor[n.type] || 'text-accent-blue'" :style="{ backgroundColor: 'currentColor' }" />
          <div class="flex-1 min-w-0">
            <p v-if="n.title" class="text-[10px] font-bold text-text-primary truncate">{{ n.title }}</p>
            <p class="text-[11px] text-text-secondary leading-relaxed break-words">{{ n.message }}</p>
            <p class="text-[9px] text-text-tertiary mt-1 opacity-60">{{ formatTime(n.createdAt) }}</p>
          </div>
          <button
            class="flex-none opacity-0 group-hover:opacity-100 text-text-tertiary hover:text-red-400 transition-all"
            @click="remove(n.id)"
          >
            <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.scrollbar-hide::-webkit-scrollbar { display: none; }
.scrollbar-hide { -ms-overflow-style: none; scrollbar-width: none; }
</style>
