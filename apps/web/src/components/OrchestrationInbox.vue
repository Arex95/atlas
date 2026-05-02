<script setup lang="ts">
import { onMounted, watch, ref, computed } from 'vue';
import { useWorkspaceStore } from '@/stores/workspace';

const props = defineProps<{
  sessionId: string;
}>();

const store = useWorkspaceStore();
const loading = ref(false);
const collapsed = ref(false);

const messages = computed(() => store.inboxMessages[props.sessionId] || []);

async function loadMessages() {
  loading.value = true;
  await store.fetchInbox(props.sessionId);
  loading.value = false;
}

onMounted(() => {
  loadMessages();
});

watch(() => props.sessionId, () => {
  loadMessages();
});

function formatTime(timestamp: string | undefined) {
  if (!timestamp) return '';

  try {
    const date = new Date(timestamp);
    const datePart = date.toISOString().split('T')[0];
    const timePart = date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
    return `${datePart} ${timePart}`;
  } catch (e) {
    return '??-??-?? ??:??:??';
  }
}

function getAgentColor(fromId: string): string {
  const colors = [
    'text-agent-1', 'text-agent-2', 'text-agent-3',
    'text-agent-4', 'text-agent-5', 'text-agent-6',
  ];
  let hash = 0;
  for (let i = 0; i < fromId.length; i++) {
    hash = fromId.charCodeAt(i) + ((hash << 5) - hash);
  }
  return colors[Math.abs(hash) % colors.length];
}
</script>

<template>
  <!-- collapsed strip -->
  <div v-if="collapsed" class="flex flex-col w-8 flex-none h-full border-l border-white/5 bg-bg-sidebar items-center py-2 gap-3">
    <button
      class="p-1.5 rounded hover:bg-white/10 text-text-tertiary hover:text-white transition-colors"
      title="Expand inbox"
      @click="collapsed = false"
    >
      <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
        <path stroke-linecap="round" stroke-linejoin="round" d="M15 19l-7-7 7-7" />
      </svg>
    </button>
    <div v-if="messages.length > 0" class="w-4 h-4 rounded-full bg-accent-blue flex items-center justify-center">
      <span class="text-[7px] font-black text-bg-primary">{{ messages.length > 9 ? '9+' : messages.length }}</span>
    </div>
    <span class="text-[8px] font-black uppercase tracking-[0.2em] text-text-tertiary opacity-50 [writing-mode:vertical-lr] select-none">
      {{ $t('inbox.title') }}
    </span>
  </div>

  <!-- expanded panel -->
  <div v-else class="w-80 flex-none border-l border-white/5 bg-bg-sidebar flex flex-col overflow-hidden animate-in fade-in slide-in-from-right duration-300">

    <div class="h-12 flex-none border-b border-white/5 px-4 flex items-center justify-between bg-black/20 backdrop-blur-sm">
      <div class="flex items-center gap-2">
        <div class="w-2 h-2 rounded-full bg-accent-blue animate-pulse shadow-[0_0_8px_rgba(59,130,246,0.5)]"></div>
        <h3 class="text-[10px] font-black uppercase tracking-[0.2em] text-text-secondary">{{ $t('inbox.title') }}</h3>
      </div>
      <div class="flex items-center gap-2">
        <span class="text-[9px] px-1.5 py-0.5 rounded bg-white/5 text-text-tertiary font-bold border border-white/5">
          {{ messages.length }}
        </span>
        <button
          class="p-1 hover:bg-white/5 rounded transition-colors text-text-tertiary hover:text-white"
          title="Collapse inbox"
          @click="collapsed = true"
        >
          <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" />
          </svg>
        </button>
      </div>
    </div>

    <div class="flex-1 overflow-y-auto scrollbar-hide p-4 space-y-6">
      <div v-if="loading && messages.length === 0" class="flex flex-col items-center justify-center h-40 opacity-20">
        <div class="w-6 h-6 border-2 border-white/30 border-t-white rounded-full animate-spin mb-3"></div>
        <span class="text-[10px] uppercase tracking-widest font-bold">{{ $t('inbox.syncing') }}</span>
      </div>

      <div v-else-if="messages.length === 0" class="flex flex-col items-center justify-center h-64 text-center">
        <div class="w-12 h-12 rounded-2xl bg-white/[0.02] border border-white/5 flex items-center justify-center mb-4 group hover:border-purple-500/30 transition-colors">
          <svg class="w-5 h-5 text-text-tertiary group-hover:text-purple-400 transition-colors" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z" />
          </svg>
        </div>
        <h4 class="text-[11px] font-bold text-text-secondary mb-1">{{ $t('inbox.noMessages') }}</h4>
        <p class="text-[10px] text-text-tertiary px-8 leading-relaxed opacity-60">{{ $t('inbox.noMessagesHint') }}</p>
      </div>

      <div v-for="msg in messages" :key="msg.id" class="group relative animate-in fade-in slide-in-from-bottom-2 duration-500">

        <div class="flex items-center gap-3 mb-2">
          <div class="h-px flex-1 bg-white/[0.03]"></div>
          <span class="text-[9px] font-mono text-text-tertiary opacity-40 group-hover:opacity-100 transition-opacity">
            {{ formatTime(msg.timestamp) }}
          </span>
        </div>

        <div class="p-3 rounded-lg bg-white/[0.02] border border-white/[0.05] group-hover:border-white/10 group-hover:bg-white/[0.03] transition-all relative overflow-hidden">

          <div class="flex items-center gap-2 mb-2">
            <div class="w-1.5 h-1.5 rounded-full" :style="{ backgroundColor: `var(--color-${getAgentColor(msg.fromId).replace('text-', '')})` }"></div>
            <span class="text-[9px] font-black tracking-wider uppercase" :class="getAgentColor(msg.fromId)">
              {{ msg.fromId }}
            </span>
          </div>

          <div class="text-[11px] leading-relaxed text-text-secondary font-mono break-words whitespace-pre-wrap">
            {{ msg.content }}
          </div>

          
          <div class="absolute -right-2 -bottom-2 opacity-[0.02] pointer-events-none group-hover:opacity-[0.05] transition-opacity">
            <svg class="w-12 h-12" fill="currentColor" viewBox="0 0 24 24">
              <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.41 0-8-3.59-8-8s3.59-8 8-8 8 3.59 8 8-3.59 8-8 8z"/>
            </svg>
          </div>
        </div>
      </div>
    </div>

    <div class="h-10 flex-none px-4 border-t border-white/5 flex items-center justify-between bg-black/10">
      <div class="flex items-center gap-1.5">
        <div class="w-1 h-1 rounded-full bg-accent-green"></div>
        <span class="text-[8px] font-bold uppercase tracking-tighter text-text-tertiary">{{ $t('inbox.realtimeActive') }}</span>
      </div>
    </div>
  </div>
  <!-- end expanded panel -->
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
