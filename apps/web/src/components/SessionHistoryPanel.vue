<script setup lang="ts">
import { computed, watch, ref } from 'vue';
import { useWorkspaceStore } from '@/stores/workspace';
import { api } from '@/api/client';

const props = defineProps<{ sessionId: string }>();

const store = useWorkspaceStore();
const collapsed = ref(false);
const loading = ref(false);

const messages = computed(() => {
  const session = store.tabs.find((t) => t.id === props.sessionId);
  return session?.history ?? [];
});

async function load(id: string) {
  loading.value = true;
  try {
    await store.fetchHistory(id);
  } finally {
    loading.value = false;
  }
}

async function removeEntry(msgId: string) {
  await api.delete(`/api/sessions/${props.sessionId}/history/${msgId}`);
  await store.fetchHistory(props.sessionId);
}

watch(() => props.sessionId, (id) => { if (id) load(id); }, { immediate: true });

function formatTime(ts: string) {
  try {
    const d = new Date(ts);
    return d.toISOString().split('T')[0] + ' ' + d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  } catch {
    return ts;
  }
}
</script>

<template>
  <!-- Collapsed strip -->
  <div v-if="collapsed" class="flex flex-col w-8 flex-none h-full border-l border-white/5 bg-bg-sidebar items-center py-2 gap-3">
    <button
      class="p-1.5 rounded hover:bg-white/10 text-text-tertiary hover:text-white transition-colors"
      :title="$t('sessionHistory.expandTitle')"
      @click="collapsed = false"
    >
      <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
        <path stroke-linecap="round" stroke-linejoin="round" d="M15 19l-7-7 7-7" />
      </svg>
    </button>
    <div v-if="messages.length > 0" class="w-4 h-4 rounded-full bg-accent-purple/80 flex items-center justify-center">
      <span class="text-[7px] font-black text-bg-primary">{{ messages.length > 9 ? '9+' : messages.length }}</span>
    </div>
    <span class="text-[8px] font-black uppercase tracking-[0.2em] text-text-tertiary opacity-50 [writing-mode:vertical-lr] select-none">
      {{ $t('sessionHistory.title') }}
    </span>
  </div>

  <!-- Expanded panel -->
  <div v-else class="w-80 flex-none border-l border-white/5 bg-bg-sidebar flex flex-col overflow-hidden animate-in fade-in slide-in-from-right duration-300">

    <div class="h-12 flex-none border-b border-white/5 px-4 flex items-center justify-between bg-black/20 backdrop-blur-sm">
      <div class="flex items-center gap-2">
        <svg class="w-3.5 h-3.5 text-accent-purple" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M8 10h.01M12 10h.01M16 10h.01M9 16H5a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v8a2 2 0 01-2 2h-5l-5 5v-5z" />
        </svg>
        <h3 class="text-[10px] font-black uppercase tracking-[0.2em] text-text-secondary">{{ $t('sessionHistory.title') }}</h3>
      </div>
      <div class="flex items-center gap-2">
        <span class="text-[9px] px-1.5 py-0.5 rounded bg-white/5 text-text-tertiary font-bold border border-white/5">
          {{ messages.length }}
        </span>
        <button
          class="p-1 hover:bg-white/5 rounded transition-colors text-text-tertiary hover:text-white"
          :title="$t('sessionHistory.collapseTitle')"
          @click="collapsed = true"
        >
          <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" />
          </svg>
        </button>
      </div>
    </div>

    <div class="flex-1 overflow-y-auto scrollbar-hide p-4 space-y-4">
      <div v-if="loading && messages.length === 0" class="flex flex-col items-center justify-center h-40 opacity-20">
        <div class="w-6 h-6 border-2 border-white/30 border-t-white rounded-full animate-spin mb-3" />
        <span class="text-[10px] uppercase tracking-widest font-bold">{{ $t('sessionHistory.loading') }}</span>
      </div>

      <div v-else-if="messages.length === 0" class="flex flex-col items-center justify-center h-64 text-center">
        <div class="w-12 h-12 rounded-2xl bg-white/[0.02] border border-white/5 flex items-center justify-center mb-4">
          <svg class="w-5 h-5 text-text-tertiary" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M8 10h.01M12 10h.01M16 10h.01M9 16H5a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v8a2 2 0 01-2 2h-5l-5 5v-5z" />
          </svg>
        </div>
        <h4 class="text-[11px] font-bold text-text-secondary mb-1">{{ $t('sessionHistory.noMessages') }}</h4>
        <p class="text-[10px] text-text-tertiary px-8 leading-relaxed opacity-60">{{ $t('sessionHistory.noMessagesHint') }}</p>
      </div>

      <div
        v-for="msg in messages"
        :key="msg.id"
        class="group animate-in fade-in slide-in-from-bottom-2 duration-500"
      >
        <div class="flex items-center gap-2 mb-1.5">
          <span
            class="text-[8px] font-black uppercase tracking-wider px-1.5 py-0.5 rounded"
            :class="{
              'bg-accent-blue/15 text-accent-blue': msg.role === 'user',
              'bg-accent-green/15 text-accent-green': msg.role === 'assistant',
              'bg-white/5 text-text-tertiary': msg.role === 'system',
            }"
          >{{ msg.role }}</span>
          <div class="h-px flex-1 bg-white/[0.03]" />
          <span class="text-[9px] font-mono text-text-tertiary opacity-40 group-hover:opacity-100 transition-opacity">
            {{ formatTime(msg.createdAt) }}
          </span>
          <button
            class="opacity-0 group-hover:opacity-100 transition-all text-text-tertiary hover:text-accent-red p-0.5 rounded"
            :title="$t('sessionHistory.deleteEntry')"
            @click="removeEntry(msg.id)"
          >
            <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
        <div class="p-3 rounded-lg bg-white/[0.02] border border-white/[0.05] group-hover:border-white/10 group-hover:bg-white/[0.03] transition-all">
          <p class="text-[11px] leading-relaxed text-text-secondary font-mono break-words whitespace-pre-wrap">{{ msg.content }}</p>
        </div>
      </div>
    </div>

    <div class="h-10 flex-none px-4 border-t border-white/5 flex items-center bg-black/10">
      <div class="w-1 h-1 rounded-full bg-accent-purple mr-1.5" />
      <span class="text-[8px] font-bold uppercase tracking-tighter text-text-tertiary">{{ $t('sessionHistory.footer') }}</span>
    </div>
  </div>
</template>

<style scoped>
.scrollbar-hide::-webkit-scrollbar { display: none; }
.scrollbar-hide { -ms-overflow-style: none; scrollbar-width: none; }
</style>
