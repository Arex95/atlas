<script setup lang="ts">
import { ref, watch, onMounted } from 'vue';
import type { Project } from '@atlas/domain';
import { formatPath } from '@/utils/path';
import { useWorkspaceStore } from '@/stores/workspace';
import { api } from '@/api/client';
import { useToast } from '@/composables/useToast';
import ProjectSessions from './ProjectSessions.vue';
import ProjectDocuments from './ProjectDocuments.vue';
import ProjectSkills from './ProjectSkills.vue';
import ProjectReminders from './ProjectReminders.vue';
import ProjectMetrics from './ProjectMetrics.vue';
import PromptLibrary from './PromptLibrary.vue';
import ProjectTasks from './ProjectTasks.vue';
import GanttView from './GanttView.vue';

const props = defineProps<{
  project: Project;
}>();

const emit = defineEmits<{
  addTerminal: []
  updateColor: [color: string]
  updateProject: [payload: {
    name?: string,
    description?: string,
    rootPath?: string,
    indexPath?: string,
    version?: string,
    author?: string
  }]
  openSession: [id: string]
}>();

type DashTab = 'overview' | 'tasks' | 'timeline' | 'sessions' | 'documents' | 'skills' | 'reminders' | 'metrics' | 'prompts';
const activeTab = ref<DashTab>('overview');
const toast = useToast();

async function copyPrompt(text: string) {
  await navigator.clipboard.writeText(text);
  toast.show('Prompt copied to clipboard', 'success');
}

interface MemoryEntry { key: string; value: string; updated_at: string }
const memory = ref<MemoryEntry[]>([]);
const isLoadingMemory = ref(false);

async function fetchMemory() {
  isLoadingMemory.value = true;
  try {
    memory.value = await api.get<MemoryEntry[]>(
      `/api/orchestrator/memory?projectId=${props.project.id}`,
    );
  } catch {
    memory.value = [];
  } finally {
    isLoadingMemory.value = false;
  }
}

async function removeMemory(key: string) {
  try {
    await api.delete(`/api/orchestrator/memory/${key}?projectId=${props.project.id}`);
    fetchMemory();
  } catch {
    // deletion failed silently; next fetchMemory will reflect actual state
  }
}

async function saveMemory(key: string) {
  try {
    await api.post('/api/orchestrator/memory', {
      projectId: props.project.id,
      key,
      value: editedMemoryValue.value,
    });
    editingMemoryKey.value = null;
    fetchMemory();
  } catch {
    // save failed silently; next fetchMemory will reflect actual state
  }
}

onMounted(() => {
  fetchMemory();
});

watch(() => props.project.id, () => {
  fetchMemory();
  activeTab.value = 'overview';
});

const isEditingName = ref(false);
const isEditingDesc = ref(false);
const isEditingPath = ref(false);
const isEditingIndex = ref(false);
const isEditingVersion = ref(false);
const isEditingAuthor = ref(false);

const editedName = ref(props.project.name);
const editedDesc = ref(props.project.description || '');
const editedPath = ref(props.project.rootPath);
const editedIndex = ref(props.project.indexPath);
const editedVersion = ref(props.project.version);
const editedAuthor = ref(props.project.author || '');

const editingMemoryKey = ref<string | null>(null);
const editedMemoryValue = ref('');

watch(() => props.project, (p) => {
  editedName.value = p.name;
  editedDesc.value = p.description || '';
  editedPath.value = p.rootPath;
  editedIndex.value = p.indexPath;
  editedVersion.value = p.version;
  editedAuthor.value = p.author || '';
}, { deep: true });

function saveName() {
  if (editedName.value.trim() && editedName.value !== props.project.name) {
    emit('updateProject', { name: editedName.value.trim() });
  }
  isEditingName.value = false;
}

function saveDesc() {
  if (editedDesc.value !== props.project.description) {
    emit('updateProject', { description: editedDesc.value.trim() });
  }
  isEditingDesc.value = false;
}

function savePath() {
  if (editedPath.value.trim() && editedPath.value !== props.project.rootPath) {
    emit('updateProject', { rootPath: editedPath.value.trim() });
  }
  isEditingPath.value = false;
}

function saveIndex() {
  if (editedIndex.value.trim() && editedIndex.value !== props.project.indexPath) {
    emit('updateProject', { indexPath: editedIndex.value.trim() });
  }
  isEditingIndex.value = false;
}

function saveVersion() {
  if (editedVersion.value.trim() && editedVersion.value !== props.project.version) {
    emit('updateProject', { version: editedVersion.value.trim() });
  }
  isEditingVersion.value = false;
}

function saveAuthor() {
  if (editedAuthor.value.trim() && editedAuthor.value !== props.project.author) {
    emit('updateProject', { author: editedAuthor.value.trim() });
  }
  isEditingAuthor.value = false;
}

const vFocus = {
  mounted: (el: HTMLInputElement) => el.focus()
};

const store = useWorkspaceStore();
const isIndexing = ref(false);

async function handleSyncIndex() {
  isIndexing.value = true;
  await store.indexProject(props.project.slug);
  isIndexing.value = false;
}

const tabs: { id: DashTab; label: string }[] = [
  { id: 'overview', label: 'projectDashboard.tabs.overview' },
  { id: 'tasks', label: 'projectDashboard.tabs.tasks' },
  { id: 'timeline', label: 'projectDashboard.tabs.timeline' },
  { id: 'sessions', label: 'projectDashboard.tabs.sessions' },
  { id: 'documents', label: 'projectDashboard.tabs.documents' },
  { id: 'skills', label: 'projectDashboard.tabs.skills' },
  { id: 'reminders', label: 'projectDashboard.tabs.reminders' },
  { id: 'metrics', label: 'projectDashboard.tabs.metrics' },
  { id: 'prompts', label: 'projectDashboard.tabs.prompts' },
];
</script>

<template>
  <div class="max-w-5xl mx-auto pb-20 px-4">
    <div class="bg-bg-sidebar/20 border border-border-primary overflow-hidden shadow-2xl">

      <div class="px-8 py-8 border-b border-border-primary bg-gradient-to-br from-bg-sidebar/40 to-transparent">
        <div class="flex items-center gap-4 mb-4">
          <div class="p-3 border rounded-lg" :style="{ backgroundColor: project.color + '1a', borderColor: project.color + '33' }">
            <svg class="w-6 h-6" :style="{ color: project.color }" fill="currentColor" viewBox="0 0 20 20">
              <path d="M2 6a2 2 0 012-2h4l2 2h4a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z"/>
            </svg>
          </div>
          <div class="flex flex-col flex-1">
            <div class="flex items-center justify-between">
              <div v-if="isEditingName" class="flex-1 mr-4">
                <input
                  v-model="editedName"
                  v-focus
                  class="w-full bg-bg-primary border border-accent-blue px-3 py-1 text-3xl font-black text-white uppercase tracking-tighter outline-none"
                  @blur="saveName"
                  @keyup.enter="saveName"
                  @keyup.esc="isEditingName = false"
                />
              </div>
              <h1
                v-else
                class="text-3xl font-black text-white tracking-tighter uppercase leading-none cursor-text hover:bg-white/5 transition-colors px-1 -ml-1 rounded"
                @click="isEditingName = true"
              >
                {{ project.name }}
              </h1>
              <div class="flex items-center gap-3 p-1.5 bg-black/20 rounded-md border border-white/5">
                <input
                  type="color"
                  :value="project.color"
                  class="w-6 h-6 bg-transparent border-none cursor-pointer rounded overflow-hidden"
                  @input="emit('updateColor', ($event.target as HTMLInputElement).value)"
                />
                <span class="text-[10px] text-text-tertiary font-mono uppercase">{{ project.color }}</span>
              </div>
            </div>
            <div class="flex items-center gap-2 mt-2">
              <span class="text-[10px] text-white px-1.5 py-0.5 font-bold uppercase tracking-widest" :style="{ backgroundColor: project.color }">{{ $t('projectDashboard.statusActive') }}</span>
              <span class="text-[10px] text-text-tertiary font-mono uppercase tracking-wider">{{ project.slug }}</span>
            </div>
          </div>
        </div>

        <div v-if="isEditingDesc" class="mt-2">
          <textarea
            v-model="editedDesc"
            v-focus
            class="w-full bg-bg-primary border border-accent-blue p-3 text-sm text-text-secondary leading-relaxed outline-none resize-none h-24"
            @blur="saveDesc"
            @keyup.esc="isEditingDesc = false"
          />
        </div>
        <p
          v-else
          class="text-sm text-text-secondary leading-relaxed max-w-2xl cursor-text hover:bg-white/5 transition-colors p-2 -m-2 rounded"
          @click="isEditingDesc = true"
        >
          {{ project.description || $t('projectDashboard.noDescription') }}
        </p>
      </div>

      <div class="flex border-b border-border-primary bg-black/20 px-8">
        <button
          v-for="tab in tabs"
          :key="tab.id"
          class="px-4 py-3 text-[10px] font-black uppercase tracking-wider transition-colors border-b-2"
          :class="activeTab === tab.id
            ? 'text-white border-accent-blue'
            : 'text-text-tertiary border-transparent hover:text-text-secondary hover:border-white/20'"
          @click="activeTab = tab.id"
        >
          {{ $t(tab.label) }}
        </button>
      </div>

      <div class="p-8 min-h-[400px]">
        <div v-if="activeTab === 'overview'" class="space-y-8">
          <div class="grid grid-cols-3 border border-border-primary bg-black/10">
            <div class="p-6 border-r border-border-primary">
              <span class="block text-[10px] font-bold text-text-tertiary uppercase tracking-widest mb-2 opacity-50">{{ $t('projectDashboard.rootDirectory') }}</span>
              <div v-if="isEditingPath">
                <input
                  v-model="editedPath"
                  v-focus
                  class="w-full bg-bg-primary border border-accent-blue px-2 py-0.5 text-[12px] text-white font-mono outline-none"
                  @blur="savePath"
                  @keyup.enter="savePath"
                  @keyup.esc="isEditingPath = false"
                />
              </div>
              <span
                v-else
                class="text-[12px] text-white font-mono truncate block cursor-text hover:bg-white/5 px-1 -ml-1 rounded transition-colors"
                @click="isEditingPath = true"
              >
                {{ formatPath(project.rootPath) }}
              </span>
            </div>

            <div class="p-6 border-r border-border-primary">
              <span class="block text-[10px] font-bold text-text-tertiary uppercase tracking-widest mb-2 opacity-50">{{ $t('projectDashboard.indexPath') }}</span>
              <div v-if="isEditingIndex">
                <input
                  v-model="editedIndex"
                  v-focus
                  class="w-full bg-bg-primary border border-accent-blue px-2 py-0.5 text-[12px] text-white font-mono outline-none"
                  @blur="saveIndex"
                  @keyup.enter="saveIndex"
                  @keyup.esc="isEditingIndex = false"
                />
              </div>
              <span
                v-else
                class="text-[12px] text-white font-mono truncate block cursor-text hover:bg-white/5 px-1 -ml-1 rounded transition-colors"
                @click="isEditingIndex = true"
              >
                {{ project.indexPath.split('/').pop() }}
              </span>
            </div>

            <div class="p-6">
              <span class="block text-[10px] font-bold text-text-tertiary uppercase tracking-widest mb-2 opacity-50">{{ $t('projectDashboard.createdAt') }}</span>
              <span class="text-[12px] text-white font-mono block opacity-60">{{ new Date(project.createdAt).toLocaleDateString() }}</span>
            </div>
          </div>

          <div class="grid grid-cols-2 gap-12">
            <div class="space-y-4">
              <h3 class="text-[11px] font-bold text-white uppercase tracking-[0.2em] flex items-center gap-2">
                <span class="w-2 h-2 rounded-full" :style="{ backgroundColor: project.color }"/>
                {{ $t('projectDashboard.environmentConfig') }}
              </h3>
              <div class="space-y-4 pt-2">
                <div class="flex justify-between items-center text-[11px] py-2 border-b border-white/5 group">
                  <span class="text-text-tertiary font-bold uppercase tracking-tighter">{{ $t('projectDashboard.version') }}</span>
                  <div v-if="isEditingVersion">
                    <input
                      v-model="editedVersion"
                      v-focus
                      class="bg-bg-primary border border-accent-blue px-2 py-0.5 text-right font-mono outline-none"
                      @blur="saveVersion"
                      @keyup.enter="saveVersion"
                      @keyup.esc="isEditingVersion = false"
                    />
                  </div>
                  <span
                    v-else
                    class="text-text-secondary font-mono cursor-text hover:text-white transition-colors"
                    @click="isEditingVersion = true"
                  >
                    {{ project.version || '0.1.0' }}
                  </span>
                </div>

                <div class="flex justify-between items-center text-[11px] py-2 border-b border-white/5 group">
                  <span class="text-text-tertiary font-bold uppercase tracking-tighter">{{ $t('projectDashboard.author') }}</span>
                  <div v-if="isEditingAuthor">
                    <input
                      v-model="editedAuthor"
                      v-focus
                      class="bg-bg-primary border border-accent-blue px-2 py-0.5 text-right font-mono outline-none"
                      @blur="saveAuthor"
                      @keyup.enter="saveAuthor"
                      @keyup.esc="isEditingAuthor = false"
                    />
                  </div>
                  <span
                    v-else
                    class="text-text-secondary font-mono cursor-text hover:text-white transition-colors"
                    @click="isEditingAuthor = true"
                  >
                    {{ project.author || $t('projectDashboard.defaultAuthor') }}
                  </span>
                </div>

                <div class="flex justify-between items-center text-[11px] py-2">
                  <span class="text-text-tertiary font-bold uppercase tracking-tighter">{{ $t('projectDashboard.status') }}</span>
                  <span class="text-accent-green font-bold tracking-widest uppercase">{{ $t('projectDashboard.statusReady') }}</span>
                </div>
              </div>
            </div>

            <div class="space-y-4">
              <h3 class="text-[11px] font-bold text-white uppercase tracking-[0.2em]">{{ $t('projectDashboard.projectActions') }}</h3>
              <div class="grid grid-cols-1 gap-3 pt-2">
                <button class="flex items-center justify-between p-3.5 bg-white hover:bg-white/90 text-black transition-all group rounded-sm" @click="emit('addTerminal')">
                  <span class="text-[11px] font-black uppercase tracking-widest">{{ $t('projectDashboard.newTerminal') }}</span>
                  <svg class="w-4 h-4 group-hover:translate-x-1 transition-transform" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="3"><path d="M14 5l7 7-7 7M3 12h18"/></svg>
                </button>
                <button :disabled="isIndexing" class="flex items-center justify-between p-3.5 bg-bg-sidebar border border-border-primary hover:border-accent-blue/50 text-text-secondary hover:text-white transition-all group rounded-sm disabled:opacity-50" @click="handleSyncIndex">
                  <div class="flex flex-col items-start">
                    <span class="text-[11px] font-bold uppercase tracking-widest text-left">{{ $t('projectDashboard.syncIndex') }}</span>
                    <span class="text-[9px] opacity-40 font-mono">{{ project.lastSyncedAt ? $t('projectDashboard.lastSynced', { date: new Date(project.lastSyncedAt).toLocaleString() }) : $t('projectDashboard.neverSynced') }}</span>
                  </div>
                  <svg :class="['w-4 h-4', isIndexing ? 'animate-spin text-accent-blue' : 'text-text-tertiary group-hover:text-accent-blue']" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5"><path d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"/></svg>
                </button>
                <button class="flex items-center justify-between p-3.5 bg-bg-sidebar border border-border-primary hover:border-text-tertiary text-text-secondary hover:text-white transition-all group rounded-sm" @click="fetchMemory">
                  <span class="text-[11px] font-bold uppercase tracking-widest text-left">{{ $t('projectDashboard.refreshIntelligence') }}</span>
                  <svg :class="['w-4 h-4', isLoadingMemory ? 'animate-spin' : '']" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5"><path d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"/></svg>
                </button>
              </div>
            </div>
          </div>

          <div class="space-y-4 pt-4 border-t border-white/5">
            <div class="flex items-center justify-between">
              <h3 class="text-[11px] font-bold text-white uppercase tracking-[0.2em] flex items-center gap-2">
                <svg class="w-4 h-4 text-accent-blue" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
                </svg>
                {{ $t('projectDashboard.memoryTitle') }}
              </h3>
              <span class="text-[10px] text-text-tertiary font-mono">{{ $t('projectDashboard.keysStored', { count: memory.length }) }}</span>
            </div>

            <div v-if="memory.length > 0" class="grid grid-cols-1 gap-2">
              <div
                v-for="item in memory"
                :key="item.key"
                class="flex items-center justify-between p-3 bg-white/5 border border-white/10 rounded group hover:border-white/20 transition-colors"
              >
                <div class="flex flex-col gap-1 flex-1">
                  <span class="text-[11px] font-bold text-accent-blue font-mono uppercase tracking-tighter">{{ item.key }}</span>
                  <div v-if="editingMemoryKey === item.key" class="flex items-center gap-2 pr-4">
                    <input
                      v-model="editedMemoryValue"
                      autofocus
                      type="text"
                      class="bg-black/40 border border-accent-blue/50 text-[12px] text-white px-2 py-1 rounded w-full outline-none focus:border-accent-blue"
                      @keyup.enter="saveMemory(item.key)"
                      @keyup.esc="editingMemoryKey = null"
                    />
                    <button class="text-accent-green hover:text-white transition-colors" @click="saveMemory(item.key)">
                      <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path d="M5 13l4 4L19 7"/></svg>
                    </button>
                  </div>
                  <span
                    v-else
                    class="text-[12px] text-text-secondary leading-relaxed cursor-pointer hover:text-white transition-colors"
                    @click="editingMemoryKey = item.key; editedMemoryValue = item.value"
                  >
                    {{ item.value }}
                  </span>
                </div>
                <div class="flex items-center gap-4">
                  <div class="text-[9px] text-text-tertiary font-mono opacity-0 group-hover:opacity-100 transition-opacity">
                    {{ $t('projectDashboard.updatedPrefix') }} {{ new Date(item.updated_at).toLocaleString() }}
                  </div>
                  <button
                    class="opacity-0 group-hover:opacity-100 text-text-tertiary hover:text-accent-red transition-all p-1"
                    :title="$t('projectDashboard.deleteMemoryKey')"
                    @click="removeMemory(item.key)"
                  >
                    <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                      <path stroke-linecap="round" stroke-linejoin="round" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                    </svg>
                  </button>
                </div>
              </div>
            </div>

            <div v-else class="p-8 border border-dashed border-white/10 rounded flex flex-col items-center justify-center gap-2 text-text-tertiary">
              <svg class="w-8 h-8 opacity-20" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M19.428 15.428a2 2 0 00-1.022-.547l-2.387-.477a6 6 0 00-3.86.517l-.318.158a6 6 0 01-3.86.517L6.05 15.21a2 2 0 00-1.806.547M8 4h8l-1 1v5.172a2 2 0 00.586 1.414l5 5c1.26 1.26.367 3.414-1.415 3.414H4.828c-1.782 0-2.674-2.154-1.414-3.414l5-5A2 2 0 009 10.172V5L8 4z" />
              </svg>
              <span class="text-[11px] uppercase tracking-widest font-bold">{{ $t('projectDashboard.noMemory') }}</span>
              <p class="text-[10px] max-w-[200px] text-center opacity-50">{{ $t('projectDashboard.noMemoryHint') }}</p>
            </div>
          </div>
        </div>

        <div v-else-if="activeTab === 'tasks'" class="h-[600px]">
          <ProjectTasks :project-id="project.id" />
        </div>

        <div v-else-if="activeTab === 'timeline'" class="h-[500px]">
          <GanttView :project-id="project.id" :project-color="project.color" />
        </div>

        <div v-else-if="activeTab === 'sessions'" class="h-[500px]">
          <ProjectSessions
            :project-id="project.id"
            @open-session="emit('openSession', $event)"
          />
        </div>

        <div v-else-if="activeTab === 'documents'" class="h-[500px]">
          <ProjectDocuments :project-id="project.id" />
        </div>

        <div v-else-if="activeTab === 'skills'" class="h-[500px]">
          <ProjectSkills :project-id="project.id" />
        </div>

        <div v-else-if="activeTab === 'reminders'" class="h-[500px]">
          <ProjectReminders :project-id="project.id" />
        </div>

        <div v-else-if="activeTab === 'metrics'">
          <ProjectMetrics :project-slug="project.slug" />
        </div>

        <div v-else-if="activeTab === 'prompts'" class="h-[500px] p-4">
          <PromptLibrary :project-id="project.id" @inject="copyPrompt" />
        </div>
      </div>

      <div class="px-8 py-4 bg-black/40 flex justify-between items-center border-t border-white/5">
        <span class="text-[9px] text-text-tertiary font-bold uppercase tracking-[0.4em] opacity-40">{{ $t('projectDashboard.footer') }}</span>
        <span class="text-[9px] text-text-tertiary/20 font-mono italic">UID: {{ project.id }}</span>
      </div>
    </div>
  </div>
</template>
