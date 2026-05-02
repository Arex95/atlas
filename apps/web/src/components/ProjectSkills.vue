<script setup lang="ts">
import { ref, onMounted } from 'vue';
import type { AgentSkill } from '@atlas/domain';
import { api } from '@/api/client';

const props = defineProps<{ projectId: string }>();

const skills = ref<AgentSkill[]>([]);
const loading = ref(false);
const selected = ref<AgentSkill | null>(null);
const showCreate = ref(false);
const saving = ref(false);
const form = ref({ name: '', description: '', script: '' });

async function load() {
  loading.value = true;
  try {
    skills.value = await api.get<AgentSkill[]>(`/api/skills?projectId=${props.projectId}`);
  } finally {
    loading.value = false;
  }
}

async function create() {
  if (!form.value.name.trim() || !form.value.script.trim()) return;
  saving.value = true;
  try {
    const skill = await api.post<AgentSkill>('/api/skills', {
      projectId: props.projectId,
      ...form.value,
    });
    skills.value.unshift(skill);
    selected.value = skill;
    showCreate.value = false;
    form.value = { name: '', description: '', script: '' };
  } finally {
    saving.value = false;
  }
}

async function remove(id: string) {
  await api.delete(`/api/skills/${id}`);
  skills.value = skills.value.filter((s) => s.id !== id);
  if (selected.value?.id === id) selected.value = null;
}

onMounted(load);
</script>

<template>
  <div class="flex h-full gap-4">
    <div class="w-60 flex-none flex flex-col gap-2">
      <div class="flex items-center justify-between">
        <span class="text-[9px] font-black uppercase tracking-widest text-text-tertiary">
          {{ $t('skills.title') }} ({{ skills.length }})
        </span>
        <button
          class="text-[9px] font-black uppercase tracking-wider text-accent-blue hover:text-accent-blue/80 transition-colors"
          @click="showCreate = !showCreate"
        >
          + {{ $t('skills.new') }}
        </button>
      </div>

      <div v-if="loading" class="flex items-center justify-center p-8 opacity-40">
        <div class="w-4 h-4 border border-accent-blue border-t-transparent rounded-full animate-spin" />
      </div>
      <div v-else-if="skills.length === 0" class="text-[10px] text-text-tertiary text-center p-4 opacity-50">
        {{ $t('skills.empty') }}
      </div>
      <div v-else class="flex-1 overflow-y-auto space-y-1 scrollbar-hide">
        <div
          v-for="skill in skills"
          :key="skill.id"
          class="group flex items-center gap-2 px-3 py-2 rounded cursor-pointer border transition-colors"
          :class="selected?.id === skill.id ? 'bg-accent-green/10 border-accent-green/20' : 'hover:bg-white/[0.03] border-transparent'"
          @click="selected = skill"
        >
          <div class="flex-1 min-w-0">
            <p class="text-[11px] font-medium text-text-primary truncate">{{ skill.name }}</p>
            <p class="text-[9px] text-text-tertiary truncate">{{ skill.description }}</p>
          </div>
          <div class="flex items-center gap-1">
            <span class="text-[8px] text-text-tertiary opacity-60">{{ skill.usageCount }}×</span>
            <button
              class="opacity-0 group-hover:opacity-100 text-text-tertiary hover:text-red-400 transition-all"
              @click.stop="remove(skill.id)"
            >
              <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" /></svg>
            </button>
          </div>
        </div>
      </div>
    </div>

    <div class="flex-1 flex flex-col min-w-0 bg-bg-elevated/30 rounded-lg border border-border-primary overflow-hidden">
      <div v-if="showCreate" class="flex-1 flex flex-col p-6 gap-4">
        <h3 class="text-[10px] font-black uppercase tracking-widest text-text-secondary">{{ $t('skills.createTitle') }}</h3>
        <input
          v-model="form.name"
          class="bg-bg-sidebar border border-border-primary rounded px-3 py-2 text-[12px] text-text-primary placeholder-text-tertiary focus:outline-none focus:border-accent-green"
          :placeholder="$t('skills.namePlaceholder')"
        />
        <input
          v-model="form.description"
          class="bg-bg-sidebar border border-border-primary rounded px-3 py-2 text-[12px] text-text-primary placeholder-text-tertiary focus:outline-none focus:border-accent-green"
          :placeholder="$t('skills.descriptionPlaceholder')"
        />
        <textarea
          v-model="form.script"
          rows="10"
          class="flex-1 bg-bg-sidebar border border-border-primary rounded px-3 py-2 text-[12px] text-text-primary font-mono placeholder-text-tertiary focus:outline-none focus:border-accent-green resize-none"
          :placeholder="$t('skills.scriptPlaceholder')"
        />
        <div class="flex gap-2">
          <button
            class="flex-1 py-2 rounded text-[10px] font-black uppercase tracking-wider bg-accent-green/20 text-accent-green hover:bg-accent-green/30 transition-colors disabled:opacity-40"
            :disabled="saving"
            @click="create"
          >
            {{ $t('skills.create') }}
          </button>
          <button
            class="px-4 py-2 rounded text-[10px] font-black uppercase tracking-wider bg-white/5 text-text-tertiary hover:text-text-primary transition-colors"
            @click="showCreate = false"
          >
            {{ $t('skills.cancel') }}
          </button>
        </div>
      </div>

      <div v-else-if="!selected" class="flex-1 flex items-center justify-center opacity-30">
        <p class="text-[11px] text-text-tertiary">{{ $t('skills.selectHint') }}</p>
      </div>

      <template v-if="selected && !showCreate">
        <div class="h-10 px-4 flex items-center justify-between border-b border-border-primary">
          <div class="flex items-center gap-2">
            <span class="text-[12px] font-bold text-text-primary">{{ selected.name }}</span>
            <span class="text-[9px] text-text-tertiary opacity-60">{{ selected.usageCount }}× used</span>
          </div>
        </div>
        <div class="px-4 py-2 border-b border-border-primary">
          <p class="text-[11px] text-text-tertiary">{{ selected.description }}</p>
        </div>
        <div class="flex-1 overflow-y-auto px-4 py-3">
          <pre class="text-[11px] text-accent-green font-mono whitespace-pre-wrap leading-relaxed">{{ selected.script }}</pre>
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.scrollbar-hide::-webkit-scrollbar { display: none; }
.scrollbar-hide { -ms-overflow-style: none; scrollbar-width: none; }
</style>
