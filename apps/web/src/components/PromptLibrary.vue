<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import type { StoredPrompt } from '@atlas/domain';
import { api } from '@/api/client';

const props = defineProps<{
  projectId?: string;
  sessionId?: string;
}>();

const emit = defineEmits<{
  inject: [text: string]
}>();

const prompts = ref<StoredPrompt[]>([]);
const loading = ref(false);
const selected = ref<StoredPrompt | null>(null);
const showCreate = ref(false);
const saving = ref(false);
const editMode = ref(false);

const form = ref({ title: '', content: '', category: 'general' });
const editForm = ref({ title: '', content: '', category: 'general' });

const categories = ['general', 'atlas', 'context', 'workflow', 'debug', 'system'];
const activeCategory = ref('all');

const categoryColor: Record<string, string> = {
  atlas: 'text-accent-blue border-accent-blue/30',
  context: 'text-purple-400 border-purple-400/30',
  workflow: 'text-accent-green border-accent-green/30',
  debug: 'text-yellow-400 border-yellow-400/30',
  system: 'text-red-400 border-red-400/30',
  general: 'text-text-tertiary border-border-primary',
};

const filtered = computed(() =>
  activeCategory.value === 'all'
    ? prompts.value
    : prompts.value.filter((p) => p.category === activeCategory.value),
);

function buildQuery() {
  const params = new URLSearchParams();
  if (props.projectId) params.set('projectId', props.projectId);
  if (props.sessionId) params.set('sessionId', props.sessionId);
  return params.toString();
}

async function load() {
  loading.value = true;
  try {
    prompts.value = await api.get<StoredPrompt[]>(`/api/prompts?${buildQuery()}`);
  } finally {
    loading.value = false;
  }
}

async function create() {
  if (!form.value.title.trim() || !form.value.content.trim()) return;
  saving.value = true;
  try {
    const p = await api.post<StoredPrompt>('/api/prompts', {
      ...form.value,
      projectId: props.projectId ?? null,
      sessionId: props.sessionId ?? null,
    });
    prompts.value.unshift(p);
    selected.value = p;
    showCreate.value = false;
    form.value = { title: '', content: '', category: 'general' };
  } finally {
    saving.value = false;
  }
}

function startEdit() {
  if (!selected.value) return;
  editForm.value = {
    title: selected.value.title,
    content: selected.value.content,
    category: selected.value.category,
  };
  editMode.value = true;
}

async function saveEdit() {
  if (!selected.value) return;
  saving.value = true;
  try {
    const updated = await api.patch<StoredPrompt>(`/api/prompts/${selected.value.id}`, editForm.value);
    const i = prompts.value.findIndex((p) => p.id === updated.id);
    if (i !== -1) prompts.value[i] = updated;
    selected.value = updated;
    editMode.value = false;
  } finally {
    saving.value = false;
  }
}

async function remove(id: string) {
  await api.delete(`/api/prompts/${id}`);
  prompts.value = prompts.value.filter((p) => p.id !== id);
  if (selected.value?.id === id) selected.value = null;
}

function selectPrompt(p: StoredPrompt) {
  selected.value = p;
  showCreate.value = false;
  editMode.value = false;
}

onMounted(load);
</script>

<template>
  <div class="flex h-full gap-4">
    <div class="w-56 flex-none flex flex-col gap-2">
      <div class="flex items-center justify-between">
        <div class="flex gap-1 flex-wrap">
          <button
            class="text-[8px] font-black uppercase tracking-wider px-1.5 py-0.5 rounded transition-colors"
            :class="activeCategory === 'all' ? 'bg-accent-blue/20 text-accent-blue' : 'text-text-tertiary hover:text-text-secondary'"
            @click="activeCategory = 'all'"
          >
            All
          </button>
          <button
            v-for="cat in categories"
            :key="cat"
            class="text-[8px] font-black uppercase tracking-wider px-1.5 py-0.5 rounded transition-colors"
            :class="activeCategory === cat ? 'bg-white/10 text-text-primary' : 'text-text-tertiary hover:text-text-secondary'"
            @click="activeCategory = cat"
          >
            {{ cat }}
          </button>
        </div>
      </div>

      <button
        class="flex items-center gap-2 px-3 py-2 rounded border border-dashed border-border-primary text-text-tertiary hover:text-accent-blue hover:border-accent-blue/30 transition-colors text-[10px] font-black uppercase tracking-wider"
        @click="showCreate = true; selected = null; editMode = false"
      >
        <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5"><path d="M12 4v16m8-8H4" /></svg>
        {{ $t('promptLibrary.new') }}
      </button>

      <div v-if="loading" class="flex items-center justify-center p-6 opacity-40">
        <div class="w-4 h-4 border border-accent-blue border-t-transparent rounded-full animate-spin" />
      </div>
      <div v-else-if="filtered.length === 0" class="text-[10px] text-text-tertiary text-center p-4 opacity-50">
        {{ $t('promptLibrary.empty') }}
      </div>
      <div v-else class="flex-1 overflow-y-auto space-y-1 scrollbar-hide">
        <div
          v-for="p in filtered"
          :key="p.id"
          class="group flex items-start gap-2 px-3 py-2 rounded cursor-pointer border transition-colors"
          :class="selected?.id === p.id
            ? 'bg-accent-blue/10 border-accent-blue/20'
            : 'hover:bg-white/[0.03] border-transparent'"
          @click="selectPrompt(p)"
        >
          <div class="flex-1 min-w-0">
            <p class="text-[11px] font-medium text-text-primary truncate leading-tight">{{ p.title }}</p>
            <span
              class="text-[7px] font-black uppercase tracking-wider px-1 border rounded mt-0.5 inline-block"
              :class="categoryColor[p.category] || categoryColor.general"
            >
              {{ p.category }}
            </span>
          </div>
          <button
            class="flex-none mt-0.5 opacity-0 group-hover:opacity-100 text-text-tertiary hover:text-red-400 transition-all"
            @click.stop="remove(p.id)"
          >
            <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" /></svg>
          </button>
        </div>
      </div>
    </div>

    <div class="flex-1 flex flex-col min-w-0 bg-bg-elevated/30 rounded-lg border border-border-primary overflow-hidden">
      <div v-if="showCreate" class="flex-1 flex flex-col p-5 gap-3">
        <h3 class="text-[10px] font-black uppercase tracking-widest text-text-secondary">{{ $t('promptLibrary.createTitle') }}</h3>
        <input
          v-model="form.title"
          class="bg-bg-sidebar border border-border-primary rounded px-3 py-2 text-[12px] text-text-primary placeholder-text-tertiary focus:outline-none focus:border-accent-blue"
          :placeholder="$t('promptLibrary.titlePlaceholder')"
        />
        <select
          v-model="form.category"
          class="bg-bg-sidebar border border-border-primary rounded px-3 py-2 text-[12px] text-text-primary focus:outline-none focus:border-accent-blue"
        >
          <option v-for="cat in categories" :key="cat" :value="cat">{{ cat }}</option>
        </select>
        <textarea
          v-model="form.content"
          rows="10"
          class="flex-1 bg-bg-sidebar border border-border-primary rounded px-3 py-2 text-[12px] text-text-primary placeholder-text-tertiary focus:outline-none focus:border-accent-blue font-mono resize-none"
          :placeholder="$t('promptLibrary.contentPlaceholder')"
        />
        <div class="flex gap-2">
          <button
            class="flex-1 py-2 rounded text-[10px] font-black uppercase tracking-wider bg-accent-blue/20 text-accent-blue hover:bg-accent-blue/30 transition-colors disabled:opacity-40"
            :disabled="saving"
            @click="create"
          >
            {{ $t('promptLibrary.create') }}
          </button>
          <button
            class="px-4 py-2 rounded text-[10px] font-black uppercase tracking-wider bg-white/5 text-text-tertiary hover:text-text-primary transition-colors"
            @click="showCreate = false"
          >
            {{ $t('promptLibrary.cancel') }}
          </button>
        </div>
      </div>

      <div v-else-if="!selected" class="flex-1 flex items-center justify-center opacity-30">
        <p class="text-[11px] text-text-tertiary">{{ $t('promptLibrary.selectHint') }}</p>
      </div>

      <template v-if="selected && !showCreate">
        <div class="h-10 px-4 flex items-center justify-between border-b border-border-primary flex-none">
          <div v-if="editMode">
            <input
              v-model="editForm.title"
              class="bg-transparent text-[12px] font-bold text-text-primary focus:outline-none border-b border-accent-blue"
            />
          </div>
          <h3 v-else class="text-[12px] font-bold text-text-primary truncate">{{ selected.title }}</h3>
          <div class="flex items-center gap-2">
            <button
              v-if="!editMode"
              class="text-[9px] font-black uppercase tracking-wider text-accent-blue hover:text-accent-blue/80 px-2 py-1 rounded transition-colors"
              @click="startEdit"
            >
              {{ $t('promptLibrary.edit') }}
            </button>
            <button
              v-if="!editMode"
              class="text-[9px] font-black uppercase tracking-wider text-accent-green hover:text-accent-green/80 px-2 py-1 rounded transition-colors"
              @click="emit('inject', selected.content)"
            >
              {{ $t('promptLibrary.inject') }}
            </button>
            <template v-if="editMode">
              <select
                v-model="editForm.category"
                class="bg-bg-sidebar border border-border-primary rounded px-2 py-0.5 text-[10px] text-text-primary focus:outline-none focus:border-accent-blue"
              >
                <option v-for="cat in categories" :key="cat" :value="cat">{{ cat }}</option>
              </select>
              <button
                class="text-[9px] font-black uppercase tracking-wider text-accent-green hover:text-accent-green/80 px-2 py-1 rounded transition-colors disabled:opacity-40"
                :disabled="saving"
                @click="saveEdit"
              >
                {{ $t('promptLibrary.save') }}
              </button>
              <button
                class="text-[9px] font-black uppercase tracking-wider text-text-tertiary hover:text-text-primary px-2 py-1 rounded transition-colors"
                @click="editMode = false"
              >
                {{ $t('promptLibrary.cancel') }}
              </button>
            </template>
          </div>
        </div>

        <textarea
          v-if="editMode"
          v-model="editForm.content"
          class="flex-1 bg-transparent px-4 py-3 text-[12px] text-text-primary font-mono resize-none focus:outline-none"
        />
        <div v-else class="flex-1 overflow-y-auto px-4 py-3 scrollbar-hide">
          <pre class="text-[11px] text-text-secondary font-mono whitespace-pre-wrap leading-relaxed">{{ selected.content }}</pre>
        </div>

        <div class="px-4 py-2 border-t border-border-primary flex items-center gap-2 flex-none bg-bg-sidebar/30">
          <span class="text-[8px] font-black uppercase tracking-wider px-1.5 py-0.5 border rounded" :class="categoryColor[selected.category] || categoryColor.general">
            {{ selected.category }}
          </span>
          <span class="text-[8px] text-text-tertiary opacity-40 font-mono">
            {{ new Date(selected.updatedAt).toLocaleString() }}
          </span>
          <div class="flex-1" />
          <span v-if="selected.projectId && selected.sessionId" class="text-[8px] text-text-tertiary opacity-40">project + session</span>
          <span v-else-if="selected.projectId" class="text-[8px] text-text-tertiary opacity-40">project</span>
          <span v-else-if="selected.sessionId" class="text-[8px] text-text-tertiary opacity-40">session</span>
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.scrollbar-hide::-webkit-scrollbar { display: none; }
.scrollbar-hide { -ms-overflow-style: none; scrollbar-width: none; }
</style>
