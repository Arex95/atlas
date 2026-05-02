<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import type { ProjectDocument } from '@atlas/domain';
import { api } from '@/api/client';

const props = defineProps<{ projectId: string }>();

const documents = ref<ProjectDocument[]>([]);
const loading = ref(false);
const activeKind = ref<string>('all');
const selected = ref<ProjectDocument | null>(null);
const editMode = ref(false);
const editContent = ref('');
const editTitle = ref('');
const showCreate = ref(false);
const createTitle = ref('');
const createContent = ref('');
const createKind = ref('document');
const saving = ref(false);

const kinds = ['all', 'document', 'skill', 'index'];

const filtered = computed(() =>
  activeKind.value === 'all'
    ? documents.value
    : documents.value.filter((d) => d.type === activeKind.value),
);

async function load() {
  loading.value = true;
  try {
    documents.value = await api.get<ProjectDocument[]>(
      `/api/documents?projectId=${props.projectId}`,
    );
  } finally {
    loading.value = false;
  }
}

function select(doc: ProjectDocument) {
  selected.value = doc;
  editMode.value = false;
  editContent.value = doc.content;
  editTitle.value = doc.title;
}

function startEdit() {
  if (!selected.value) return;
  editMode.value = true;
  editContent.value = selected.value.content;
  editTitle.value = selected.value.title;
}

async function saveEdit() {
  if (!selected.value) return;
  saving.value = true;
  try {
    const updated = await api.patch<ProjectDocument>(`/api/documents/${selected.value.id}`, {
      title: editTitle.value,
      content: editContent.value,
    });
    const i = documents.value.findIndex((d) => d.id === updated.id);
    if (i !== -1) documents.value[i] = updated;
    selected.value = updated;
    editMode.value = false;
  } finally {
    saving.value = false;
  }
}

async function createDoc() {
  if (!createTitle.value.trim()) return;
  saving.value = true;
  try {
    const doc = await api.post<ProjectDocument>('/api/documents', {
      projectId: props.projectId,
      title: createTitle.value,
      content: createContent.value,
      type: createKind.value,
    });
    documents.value.unshift(doc);
    selected.value = doc;
    showCreate.value = false;
    createTitle.value = '';
    createContent.value = '';
    createKind.value = 'document';
  } finally {
    saving.value = false;
  }
}

async function remove(id: string) {
  await api.delete(`/api/documents/${id}`);
  documents.value = documents.value.filter((d) => d.id !== id);
  if (selected.value?.id === id) selected.value = null;
}

const kindBadge: Record<string, string> = {
  document: 'text-accent-blue border-accent-blue/30',
  skill: 'text-accent-green border-accent-green/30',
  index: 'text-yellow-400 border-yellow-400/30',
};

onMounted(load);
</script>

<template>
  <div class="flex h-full gap-4">
    <div class="w-60 flex-none flex flex-col gap-2">
      <div class="flex items-center justify-between">
        <div class="flex gap-1">
          <button
            v-for="k in kinds"
            :key="k"
            class="text-[9px] font-black uppercase tracking-wider px-2 py-1 rounded transition-colors"
            :class="activeKind === k ? 'bg-accent-blue/20 text-accent-blue' : 'text-text-tertiary hover:text-text-secondary'"
            @click="activeKind = k"
          >
            {{ k }}
          </button>
        </div>
        <button
          class="text-[9px] font-black uppercase tracking-wider text-accent-blue hover:text-accent-blue/80 transition-colors"
          @click="showCreate = true"
        >
          + {{ $t('documents.new') }}
        </button>
      </div>

      <div v-if="loading" class="flex items-center justify-center p-8 opacity-40">
        <div class="w-4 h-4 border border-accent-blue border-t-transparent rounded-full animate-spin" />
      </div>
      <div v-else-if="filtered.length === 0" class="text-[10px] text-text-tertiary text-center p-4 opacity-50">
        {{ $t('documents.empty') }}
      </div>
      <div v-else class="flex-1 overflow-y-auto space-y-1 scrollbar-hide">
        <div
          v-for="doc in filtered"
          :key="doc.id"
          class="group flex items-center gap-2 px-3 py-2 rounded cursor-pointer transition-colors"
          :class="selected?.id === doc.id ? 'bg-accent-blue/10 border border-accent-blue/20' : 'hover:bg-white/[0.03] border border-transparent'"
          @click="select(doc)"
        >
          <div class="flex-1 min-w-0">
            <p class="text-[11px] font-medium text-text-primary truncate">{{ doc.title }}</p>
            <span class="text-[8px] font-black uppercase tracking-wider px-1 border rounded" :class="kindBadge[doc.type] || 'text-text-tertiary border-border-primary'">
              {{ doc.type }}
            </span>
          </div>
          <button
            class="flex-none opacity-0 group-hover:opacity-100 text-text-tertiary hover:text-red-400 transition-all"
            @click.stop="remove(doc.id)"
          >
            <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" /></svg>
          </button>
        </div>
      </div>
    </div>

    <div class="flex-1 flex flex-col min-w-0 bg-bg-elevated/30 rounded-lg border border-border-primary overflow-hidden">
      <div v-if="!selected && !showCreate" class="flex-1 flex items-center justify-center opacity-30">
        <p class="text-[11px] text-text-tertiary">{{ $t('documents.selectHint') }}</p>
      </div>

      <div v-if="showCreate" class="flex-1 flex flex-col p-6 gap-4">
        <h3 class="text-[10px] font-black uppercase tracking-widest text-text-secondary">{{ $t('documents.createTitle') }}</h3>
        <input
          v-model="createTitle"
          class="bg-bg-sidebar border border-border-primary rounded px-3 py-2 text-[12px] text-text-primary placeholder-text-tertiary focus:outline-none focus:border-accent-blue"
          :placeholder="$t('documents.titlePlaceholder')"
        />
        <select
          v-model="createKind"
          class="bg-bg-sidebar border border-border-primary rounded px-3 py-2 text-[12px] text-text-primary focus:outline-none focus:border-accent-blue"
        >
          <option value="document">Document</option>
          <option value="skill">Skill</option>
          <option value="index">Index</option>
        </select>
        <textarea
          v-model="createContent"
          rows="8"
          class="flex-1 bg-bg-sidebar border border-border-primary rounded px-3 py-2 text-[12px] text-text-primary placeholder-text-tertiary focus:outline-none focus:border-accent-blue font-mono resize-none"
          :placeholder="$t('documents.contentPlaceholder')"
        />
        <div class="flex gap-2">
          <button
            class="flex-1 py-2 rounded text-[10px] font-black uppercase tracking-wider bg-accent-blue/20 text-accent-blue hover:bg-accent-blue/30 transition-colors disabled:opacity-40"
            :disabled="saving"
            @click="createDoc"
          >
            {{ $t('documents.create') }}
          </button>
          <button
            class="px-4 py-2 rounded text-[10px] font-black uppercase tracking-wider bg-white/5 text-text-tertiary hover:text-text-primary transition-colors"
            @click="showCreate = false"
          >
            {{ $t('documents.cancel') }}
          </button>
        </div>
      </div>

      <template v-if="selected && !showCreate">
        <div class="h-10 px-4 flex items-center justify-between border-b border-border-primary">
          <div v-if="editMode">
            <input
              v-model="editTitle"
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
              {{ $t('documents.edit') }}
            </button>
            <template v-if="editMode">
              <button
                class="text-[9px] font-black uppercase tracking-wider text-accent-green hover:text-accent-green/80 px-2 py-1 rounded transition-colors disabled:opacity-40"
                :disabled="saving"
                @click="saveEdit"
              >
                {{ $t('documents.save') }}
              </button>
              <button
                class="text-[9px] font-black uppercase tracking-wider text-text-tertiary hover:text-text-primary px-2 py-1 rounded transition-colors"
                @click="editMode = false"
              >
                {{ $t('documents.cancel') }}
              </button>
            </template>
          </div>
        </div>

        <textarea
          v-if="editMode"
          v-model="editContent"
          class="flex-1 bg-transparent px-4 py-3 text-[12px] text-text-primary font-mono resize-none focus:outline-none"
        />
        <div v-else class="flex-1 overflow-y-auto px-4 py-3">
          <pre class="text-[11px] text-text-secondary font-mono whitespace-pre-wrap leading-relaxed">{{ selected.content || $t('documents.noContent') }}</pre>
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.scrollbar-hide::-webkit-scrollbar { display: none; }
.scrollbar-hide { -ms-overflow-style: none; scrollbar-width: none; }
</style>
