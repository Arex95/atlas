<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import type { AtlasReminder } from '@atlas/domain';
import { api } from '@/api/client';

const props = defineProps<{ projectId: string }>();

const reminders = ref<AtlasReminder[]>([]);
const loading = ref(false);
const saving = ref(false);
const showCreate = ref(false);
const form = ref({ title: '', description: '', dueAt: '', type: 'reminder' });

const pending = computed(() => reminders.value.filter((r) => r.status === 'pending'));
const done = computed(() => reminders.value.filter((r) => r.status !== 'pending'));

async function load() {
  loading.value = true;
  try {
    reminders.value = await api.get<AtlasReminder[]>(`/api/reminders?projectId=${props.projectId}`);
  } finally {
    loading.value = false;
  }
}

async function create() {
  if (!form.value.title.trim() || !form.value.dueAt) return;
  saving.value = true;
  try {
    const r = await api.post<AtlasReminder>('/api/reminders', {
      ...form.value,
      projectId: props.projectId,
    });
    reminders.value.push(r);
    showCreate.value = false;
    form.value = { title: '', description: '', dueAt: '', type: 'reminder' };
  } finally {
    saving.value = false;
  }
}

async function markDone(id: string) {
  const updated = await api.patch<AtlasReminder>(`/api/reminders/${id}`, { status: 'done' });
  const i = reminders.value.findIndex((r) => r.id === id);
  if (i !== -1) reminders.value[i] = updated;
}

async function remove(id: string) {
  await api.delete(`/api/reminders/${id}`);
  reminders.value = reminders.value.filter((r) => r.id !== id);
}

function formatDue(ts: string) {
  try {
    return new Date(ts).toLocaleString([], { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
  } catch {
    return ts;
  }
}

function isPast(ts: string) {
  return new Date(ts) < new Date();
}

onMounted(load);
</script>

<template>
  <div class="flex flex-col gap-4 h-full">
    <div class="flex items-center justify-between">
      <span class="text-[9px] font-black uppercase tracking-widest text-text-tertiary">
        {{ $t('reminders.pending') }} ({{ pending.length }})
      </span>
      <button
        class="text-[9px] font-black uppercase tracking-wider text-accent-blue hover:text-accent-blue/80 transition-colors"
        @click="showCreate = !showCreate"
      >
        + {{ $t('reminders.new') }}
      </button>
    </div>

    <div v-if="showCreate" class="bg-bg-elevated/30 border border-border-primary rounded-lg p-4 flex flex-col gap-3">
      <h3 class="text-[10px] font-black uppercase tracking-widest text-text-secondary">{{ $t('reminders.createTitle') }}</h3>
      <input
        v-model="form.title"
        class="bg-bg-sidebar border border-border-primary rounded px-3 py-2 text-[12px] text-text-primary placeholder-text-tertiary focus:outline-none focus:border-accent-blue"
        :placeholder="$t('reminders.titlePlaceholder')"
      />
      <input
        v-model="form.description"
        class="bg-bg-sidebar border border-border-primary rounded px-3 py-2 text-[12px] text-text-primary placeholder-text-tertiary focus:outline-none focus:border-accent-blue"
        :placeholder="$t('reminders.descriptionPlaceholder')"
      />
      <input
        v-model="form.dueAt"
        type="datetime-local"
        class="bg-bg-sidebar border border-border-primary rounded px-3 py-2 text-[12px] text-text-primary focus:outline-none focus:border-accent-blue"
      />
      <div class="flex gap-2">
        <button
          class="flex-1 py-2 rounded text-[10px] font-black uppercase tracking-wider bg-accent-blue/20 text-accent-blue hover:bg-accent-blue/30 transition-colors disabled:opacity-40"
          :disabled="saving"
          @click="create"
        >
          {{ $t('reminders.create') }}
        </button>
        <button
          class="px-4 py-2 rounded text-[10px] font-black uppercase tracking-wider bg-white/5 text-text-tertiary hover:text-text-primary transition-colors"
          @click="showCreate = false"
        >
          {{ $t('reminders.cancel') }}
        </button>
      </div>
    </div>

    <div v-if="loading" class="flex items-center justify-center p-8 opacity-40">
      <div class="w-4 h-4 border border-accent-blue border-t-transparent rounded-full animate-spin" />
    </div>

    <div v-else-if="pending.length === 0 && done.length === 0" class="flex items-center justify-center p-12 opacity-30">
      <p class="text-[11px] text-text-tertiary">{{ $t('reminders.empty') }}</p>
    </div>

    <div v-else class="flex-1 overflow-y-auto space-y-2 scrollbar-hide">
      <div
        v-for="r in pending"
        :key="r.id"
        class="group flex items-start gap-3 px-4 py-3 rounded-lg border transition-colors"
        :class="isPast(r.dueAt) ? 'border-red-500/30 bg-red-500/5' : 'border-border-primary bg-bg-elevated/20'"
      >
        <button
          class="flex-none w-4 h-4 rounded border mt-0.5 transition-colors hover:border-accent-green hover:bg-accent-green/20"
          :class="isPast(r.dueAt) ? 'border-red-400' : 'border-border-primary'"
          @click="markDone(r.id)"
        />
        <div class="flex-1 min-w-0">
          <p class="text-[12px] font-medium text-text-primary">{{ r.title }}</p>
          <p v-if="r.description" class="text-[10px] text-text-tertiary mt-0.5">{{ r.description }}</p>
          <p class="text-[9px] mt-1 font-mono" :class="isPast(r.dueAt) ? 'text-red-400' : 'text-text-tertiary opacity-60'">
            {{ formatDue(r.dueAt) }}
            <span v-if="isPast(r.dueAt)" class="ml-1 uppercase font-black">{{ $t('reminders.overdue') }}</span>
          </p>
        </div>
        <button
          class="flex-none opacity-0 group-hover:opacity-100 text-text-tertiary hover:text-red-400 transition-all"
          @click="remove(r.id)"
        >
          <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" /></svg>
        </button>
      </div>

      <div v-if="done.length > 0" class="pt-2">
        <p class="text-[9px] font-black uppercase tracking-widest text-text-tertiary opacity-40 mb-2">{{ $t('reminders.completed') }}</p>
        <div
          v-for="r in done"
          :key="r.id"
          class="group flex items-start gap-3 px-4 py-3 rounded-lg border border-border-primary opacity-40 hover:opacity-60 transition-opacity"
        >
          <div class="flex-none w-4 h-4 rounded border border-accent-green bg-accent-green/20 mt-0.5 flex items-center justify-center">
            <svg class="w-2.5 h-2.5 text-accent-green" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="3"><path d="M5 13l4 4L19 7" /></svg>
          </div>
          <div class="flex-1 min-w-0">
            <p class="text-[12px] text-text-tertiary line-through">{{ r.title }}</p>
          </div>
          <button
            class="flex-none opacity-0 group-hover:opacity-100 text-text-tertiary hover:text-red-400 transition-all"
            @click="remove(r.id)"
          >
            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M6 18L18 6M6 6l12 12" /></svg>
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
