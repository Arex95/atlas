<script setup lang="ts">
import { computed, ref } from 'vue';
import { useWorkspaceStore } from '@/stores/workspace';
import ConfirmDeleteModal from '@/components/ConfirmDeleteModal.vue';

const props = defineProps<{ projectId: string }>();
const emit = defineEmits<{ 'open-session': [id: string] }>();

const store = useWorkspaceStore();

const sessions = computed(() =>
  store.tabs.filter((t) => t.projectId === props.projectId),
);

const showDeleteModal = ref(false);
const pendingDeleteId = ref<string | null>(null);

function requestDeleteSession(id: string) {
  pendingDeleteId.value = id;
  showDeleteModal.value = true;
}

async function onDeleteConfirmed() {
  if (pendingDeleteId.value) {
    await store.deleteSession(pendingDeleteId.value);
  }
  showDeleteModal.value = false;
  pendingDeleteId.value = null;
}

function statusColor(status: string) {
  switch (status) {
    case 'running': return 'text-accent-green';
    case 'starting': return 'text-yellow-400';
    case 'stopped': return 'text-text-tertiary';
    default: return 'text-text-tertiary';
  }
}

function formatDate(ts: string) {
  try {
    return new Date(ts).toLocaleString([], { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
  } catch {
    return ts;
  }
}
</script>

<template>
  <div class="flex flex-col gap-2 h-full">
    <div class="flex items-center justify-between mb-2">
      <span class="text-[9px] font-black uppercase tracking-widest text-text-tertiary">
        {{ $t('projectSessions.title') }} ({{ sessions.length }})
      </span>
    </div>

    <div v-if="sessions.length === 0" class="flex items-center justify-center p-12 opacity-30">
      <p class="text-[11px] text-text-tertiary">{{ $t('projectSessions.empty') }}</p>
    </div>

    <div v-else class="flex-1 overflow-y-auto space-y-2 scrollbar-hide">
      <div
        v-for="session in sessions"
        :key="session.id"
        class="group flex items-center gap-3 px-4 py-3 rounded-lg border border-border-primary bg-bg-elevated/20 hover:border-white/10 transition-colors cursor-pointer"
        @click="emit('open-session', session.id)"
      >
        <div
          class="flex-none w-2 h-2 rounded-full"
          :class="statusColor(session.status)"
        />
        <div class="flex-1 min-w-0">
          <p class="text-[12px] font-medium text-text-primary truncate">
            {{ session.customName || session.title || session.id.slice(-8) }}
          </p>
          <p class="text-[9px] text-text-tertiary opacity-60">
            {{ session.provider }} · {{ formatDate(session.startedAt) }}
          </p>
        </div>
        <div class="flex items-center gap-2">
          <span v-if="session.isSaved" class="text-[8px] font-black uppercase px-1.5 py-0.5 rounded border border-accent-blue/30 text-accent-blue">
            {{ $t('projectSessions.saved') }}
          </span>
          <span class="text-[9px] font-black uppercase tracking-wider" :class="statusColor(session.status)">
            {{ session.status }}
          </span>
          <button
            class="opacity-0 group-hover:opacity-100 text-text-tertiary hover:text-red-400 transition-all ml-1"
            @click.stop="requestDeleteSession(session.id)"
          >
            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
            </svg>
          </button>
        </div>
      </div>
    </div>
  </div>

  <ConfirmDeleteModal
    v-if="showDeleteModal && pendingDeleteId"
    :name="sessions.find(s => s.id === pendingDeleteId)?.customName || sessions.find(s => s.id === pendingDeleteId)?.title || pendingDeleteId || ''"
    :id="pendingDeleteId || ''"
    @close="showDeleteModal = false; pendingDeleteId = null"
    @confirm="onDeleteConfirmed"
  />
</template>

<style scoped>
.scrollbar-hide::-webkit-scrollbar { display: none; }
.scrollbar-hide { -ms-overflow-style: none; scrollbar-width: none; }
</style>
