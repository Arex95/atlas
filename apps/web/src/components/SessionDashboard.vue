<script setup lang="ts">
import { ref, watch, onMounted, computed } from 'vue';
import type { AISession } from '@atlas/domain';
import PromptLibrary from './PromptLibrary.vue';
import GanttView from './GanttView.vue';
import { useToast } from '@/composables/useToast';
import { api } from '@/api/client';

const props = defineProps<{
  session: AISession;
}>();

const emit = defineEmits<{
  updateSession: [payload: { customName?: string, customDescription?: string, color?: string, linkedTaskId?: string }]
  close: []
}>();

// ── Editing ───────────────────────────────────────────────────────────────────
const isEditingName = ref(false);
const isEditingDesc = ref(false);
const editedName = ref(props.session.customName || props.session.title || props.session.model);
const editedDesc = ref(props.session.customDescription || '');

watch(() => props.session, (s) => {
  editedName.value = s.customName || s.title || s.model;
  editedDesc.value = s.customDescription || '';
}, { deep: true });

function saveName() {
  if (editedName.value.trim()) emit('updateSession', { customName: editedName.value.trim() });
  isEditingName.value = false;
}
function saveDesc() {
  emit('updateSession', { customDescription: editedDesc.value.trim() });
  isEditingDesc.value = false;
}
const vFocus = { mounted: (el: HTMLInputElement) => el.focus() };

// ── Toast / clipboard ─────────────────────────────────────────────────────────
const toast = useToast();
async function copyPrompt(text: string) {
  await navigator.clipboard.writeText(text);
  toast.show('Prompt copied to clipboard', 'success');
}

// ── Linked task ───────────────────────────────────────────────────────────────
interface LinkedTask { id: string; title: string; status: string; priority: string; dueDate?: string }
const linkedTask = ref<LinkedTask | null>(null);
const allTasks = ref<LinkedTask[]>([]);
const showTaskPicker = ref(false);

async function fetchLinkedTask() {
  try {
    allTasks.value = await api.get<LinkedTask[]>(`/api/tasks?projectId=${props.session.projectId}`);
    linkedTask.value = props.session.linkedTaskId
      ? (allTasks.value.find(t => t.id === props.session.linkedTaskId) ?? null)
      : null;
  } catch { linkedTask.value = null; }
}
async function linkTask(taskId: string | null) {
  emit('updateSession', { linkedTaskId: taskId ?? undefined } as { linkedTaskId?: string });
  showTaskPicker.value = false;
}
onMounted(fetchLinkedTask);
watch(() => props.session.linkedTaskId, fetchLinkedTask);

// ── Tabs ──────────────────────────────────────────────────────────────────────
type Tab = 'overview' | 'memory' | 'documents' | 'tasks' | 'reminders' | 'timeline';
const activeTab = ref<Tab>('overview');

// Session memory
interface MemoryRow { key: string; value: string; updated_at: string }
const memory = ref<MemoryRow[]>([]);
const memoryLoading = ref(false);
async function fetchMemory() {
  memoryLoading.value = true;
  try { memory.value = await api.get<MemoryRow[]>(`/api/sessions/${props.session.id}/memory`); }
  catch { memory.value = []; }
  finally { memoryLoading.value = false; }
}
async function deleteMemoryKey(key: string) {
  await api.delete(`/api/sessions/${props.session.id}/memory/${encodeURIComponent(key)}`);
  memory.value = memory.value.filter(m => m.key !== key);
}

// Session documents
interface DocRow { id: string; title: string; content: string; kind: string; created_at: string; updated_at: string }
const documents = ref<DocRow[]>([]);
const docsLoading = ref(false);
const expandedDoc = ref<string | null>(null);
async function fetchDocuments() {
  docsLoading.value = true;
  try { documents.value = await api.get<DocRow[]>(`/api/sessions/${props.session.id}/documents`); }
  catch { documents.value = []; }
  finally { docsLoading.value = false; }
}
async function deleteDocument(id: string) {
  await api.delete(`/api/sessions/${props.session.id}/documents/${id}`);
  documents.value = documents.value.filter(d => d.id !== id);
  if (expandedDoc.value === id) expandedDoc.value = null;
}

// Session tasks
interface TaskRow { id: string; title: string; status: string; priority: string; due_date?: string; assigned_to?: string }
const sessionTasks = ref<TaskRow[]>([]);
const tasksLoading = ref(false);
async function fetchTasks() {
  tasksLoading.value = true;
  try { sessionTasks.value = await api.get<TaskRow[]>(`/api/sessions/${props.session.id}/tasks`); }
  catch { sessionTasks.value = []; }
  finally { tasksLoading.value = false; }
}

// Session reminders
interface ReminderRow { id: string; title: string; due_at: string; status: string }
const sessionReminders = ref<ReminderRow[]>([]);
const remindersLoading = ref(false);
async function fetchReminders() {
  remindersLoading.value = true;
  try { sessionReminders.value = await api.get<ReminderRow[]>(`/api/sessions/${props.session.id}/reminders`); }
  catch { sessionReminders.value = []; }
  finally { remindersLoading.value = false; }
}

const TABS: { id: Tab; label: string }[] = [
  { id: 'overview', label: 'Overview' },
  { id: 'memory', label: 'Memory' },
  { id: 'documents', label: 'Documents' },
  { id: 'tasks', label: 'Tasks' },
  { id: 'timeline', label: 'Timeline' },
  { id: 'reminders', label: 'Reminders' },
];

const tabBadge = computed(() => ({
  memory: memory.value.length || null,
  documents: documents.value.length || null,
  tasks: sessionTasks.value.length || null,
  reminders: sessionReminders.value.length || null,
}));

async function switchTab(tab: Tab) {
  activeTab.value = tab;
  if (tab === 'memory' && memory.value.length === 0 && !memoryLoading.value) fetchMemory();
  if (tab === 'documents' && documents.value.length === 0 && !docsLoading.value) fetchDocuments();
  if (tab === 'tasks' && sessionTasks.value.length === 0 && !tasksLoading.value) fetchTasks();
  if (tab === 'reminders' && sessionReminders.value.length === 0 && !remindersLoading.value) fetchReminders();
}

// Preload counts on mount for badges
onMounted(async () => {
  await Promise.allSettled([fetchMemory(), fetchDocuments(), fetchTasks(), fetchReminders()]);
});

const priorityClass: Record<string, string> = {
  critical: 'bg-accent-red/20 text-accent-red',
  high: 'bg-accent-yellow/20 text-accent-yellow',
  medium: 'bg-accent-blue/20 text-accent-blue',
  low: 'bg-text-tertiary/20 text-text-tertiary',
};
const statusClass: Record<string, string> = {
  done: 'bg-accent-green/20 text-accent-green',
  'in-progress': 'bg-accent-blue/20 text-accent-blue',
  blocked: 'bg-accent-red/20 text-accent-red',
  todo: 'bg-text-tertiary/20 text-text-tertiary',
  pending: 'bg-accent-yellow/20 text-accent-yellow',
  completed: 'bg-accent-green/20 text-accent-green',
};
</script>

<template>
  <div class="max-w-4xl mx-auto pb-20 px-4">
    <div class="bg-bg-sidebar/20 border border-border-primary overflow-hidden shadow-2xl">

      <!-- Header -->
      <div class="px-8 py-8 border-b border-border-primary bg-gradient-to-br from-bg-sidebar/40 to-transparent">
        <div class="flex items-center gap-4 mb-4">
          <div class="p-3 border rounded-lg bg-accent-green/10 border-accent-green/30">
            <svg class="w-6 h-6 text-accent-green" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M8 9l3 3-3 3m5 0h3M5 20h14a2 2 0 002-2V6a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z" />
            </svg>
          </div>
          <div class="flex flex-col flex-1">
            <div class="flex items-center justify-between">
              <div v-if="isEditingName" class="flex-1 mr-4">
                <input
                  v-model="editedName"
                  @blur="saveName"
                  @keyup.enter="saveName"
                  @keyup.esc="isEditingName = false"
                  class="w-full bg-bg-primary border border-accent-green px-3 py-1 text-3xl font-black text-white uppercase tracking-tighter outline-none"
                  v-focus
                />
              </div>
              <h1
                v-else
                @click="isEditingName = true"
                class="text-3xl font-black text-white tracking-tighter uppercase leading-none cursor-text hover:bg-white/5 transition-colors px-1 -ml-1 rounded"
              >
                {{ session.customName || session.title || session.model }}
              </h1>
              <button @click="emit('close')" class="text-text-tertiary hover:text-white transition-colors">
                <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M6 18L18 6M6 6l12 12"/></svg>
              </button>
            </div>
            <div class="flex items-center gap-2 mt-2">
              <span class="text-[10px] bg-accent-green text-black px-1.5 py-0.5 font-bold uppercase tracking-widest">{{ $t('sessionDashboard.activeSession') }}</span>
              <span class="text-[10px] text-text-tertiary font-mono uppercase tracking-wider">{{ session.id }}</span>
            </div>
          </div>
        </div>

        <div v-if="isEditingDesc" class="mt-2">
          <textarea
            v-model="editedDesc"
            @blur="saveDesc"
            @keyup.esc="isEditingDesc = false"
            class="w-full bg-bg-primary border border-accent-green p-3 text-sm text-text-secondary leading-relaxed outline-none resize-none h-24"
            v-focus
          ></textarea>
        </div>
        <p
          v-else
          @click="isEditingDesc = true"
          class="text-sm text-text-secondary leading-relaxed max-w-2xl cursor-text hover:bg-white/5 transition-colors p-2 -m-2 rounded"
        >
          {{ session.customDescription || $t('sessionDashboard.noDescription') }}
        </p>
      </div>

      <!-- Meta strip -->
      <div class="grid grid-cols-3 border-b border-border-primary bg-black/10">
        <div class="p-6 border-r border-border-primary">
          <span class="block text-[10px] font-bold text-text-tertiary uppercase tracking-widest mb-2 opacity-50">{{ $t('sessionDashboard.model') }}</span>
          <span class="text-[12px] text-white font-mono block">{{ session.model }}</span>
        </div>
        <div class="p-6 border-r border-border-primary">
          <span class="block text-[10px] font-bold text-text-tertiary uppercase tracking-widest mb-2 opacity-50">{{ $t('sessionDashboard.provider') }}</span>
          <span class="text-[12px] text-white font-mono block uppercase">{{ session.provider }}</span>
        </div>
        <div class="p-6">
          <span class="block text-[10px] font-bold text-text-tertiary uppercase tracking-widest mb-2 opacity-50">{{ $t('sessionDashboard.startedAt') }}</span>
          <span class="text-[12px] text-white font-mono block opacity-60">{{ new Date(session.startedAt).toLocaleString() }}</span>
        </div>
      </div>

      <!-- Tab bar -->
      <div class="flex border-b border-border-primary bg-black/20">
        <button
          v-for="tab in TABS"
          :key="tab.id"
          @click="switchTab(tab.id)"
          class="flex items-center gap-2 px-6 py-3 text-[10px] font-black uppercase tracking-widest transition-colors border-b-2"
          :class="activeTab === tab.id
            ? 'text-white border-accent-green'
            : 'text-text-tertiary border-transparent hover:text-text-secondary hover:border-border-primary'"
        >
          {{ tab.label }}
          <span
            v-if="tab.id !== 'overview' && tabBadge[tab.id as keyof typeof tabBadge]"
            class="text-[9px] bg-accent-green/20 text-accent-green px-1.5 py-0.5 rounded-full font-bold"
          >
            {{ tabBadge[tab.id as keyof typeof tabBadge] }}
          </span>
        </button>
      </div>

      <!-- ── OVERVIEW TAB ──────────────────────────────────────────────── -->
      <template v-if="activeTab === 'overview'">
        <!-- Linked task -->
        <div class="px-8 py-4 border-b border-border-primary" :class="linkedTask ? 'bg-accent-blue/5' : ''">
          <div class="flex items-center gap-3">
            <svg class="w-4 h-4 text-accent-blue flex-none" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2"/>
            </svg>
            <span class="text-[10px] font-black uppercase tracking-widest text-accent-blue opacity-80">Linked Task</span>
            <template v-if="linkedTask">
              <span class="text-[12px] font-bold text-text-primary">{{ linkedTask.title }}</span>
              <span class="text-[9px] px-1.5 py-0.5 rounded font-bold uppercase" :class="statusClass[linkedTask.status] ?? 'bg-text-tertiary/20 text-text-tertiary'">{{ linkedTask.status }}</span>
              <span v-if="linkedTask.dueDate" class="text-[9px] font-mono text-accent-yellow/70">due {{ linkedTask.dueDate }}</span>
              <button @click="linkTask(null)" class="ml-auto text-[9px] font-mono text-text-tertiary hover:text-accent-red transition-colors">unlink</button>
            </template>
            <template v-else>
              <span class="text-[11px] text-text-tertiary font-mono">None</span>
              <button @click="showTaskPicker = !showTaskPicker" class="ml-auto text-[9px] font-mono text-accent-blue hover:text-white transition-colors">+ Link task</button>
            </template>
          </div>
          <div v-if="showTaskPicker && allTasks.length > 0" class="mt-3 bg-bg-elevated border border-border-primary rounded-lg overflow-hidden">
            <button
              v-for="task in allTasks.filter(t => t.status !== 'done')"
              :key="task.id"
              class="w-full flex items-center gap-3 px-4 py-2.5 text-left hover:bg-bg-sidebar/40 transition-colors border-b border-border-primary/30 last:border-0"
              @click="linkTask(task.id)"
            >
              <span class="text-[10px] font-bold text-text-primary flex-1 truncate">{{ task.title }}</span>
              <span class="text-[8px] px-1 py-0.5 rounded font-bold uppercase" :class="statusClass[task.status] ?? ''">{{ task.status }}</span>
            </button>
            <div v-if="allTasks.filter(t => t.status !== 'done').length === 0" class="px-4 py-3 text-[10px] text-text-tertiary font-mono">No open tasks</div>
          </div>
        </div>

        <!-- Working environment -->
        <div class="p-8 border-b border-border-primary">
          <h3 class="text-[11px] font-bold text-white uppercase tracking-[0.2em] mb-4">{{ $t('sessionDashboard.workingEnvironment') }}</h3>
          <div class="bg-black/40 p-4 rounded border border-white/5 font-mono text-[11px]">
            <div class="flex items-center gap-2 mb-2">
              <span class="text-text-tertiary">{{ $t('sessionDashboard.pwdLabel') }}</span>
              <span class="text-accent-green">{{ session.workingDirectory }}</span>
            </div>
            <div class="flex items-center gap-2">
              <span class="text-text-tertiary">{{ $t('sessionDashboard.modeLabel') }}</span>
              <span class="text-white uppercase">{{ session.mode }}</span>
            </div>
          </div>
        </div>

        <!-- Session actions -->
        <div class="p-8">
          <h3 class="text-[11px] font-bold text-white uppercase tracking-[0.2em] mb-4">{{ $t('sessionDashboard.sessionActions') }}</h3>
          <button @click="emit('close')" class="flex items-center justify-between p-4 bg-white hover:bg-white/90 text-black transition-all group rounded-sm w-full">
            <span class="text-[11px] font-black uppercase tracking-widest">{{ $t('sessionDashboard.returnToTerminal') }}</span>
            <svg class="w-4 h-4 group-hover:translate-x-1 transition-transform" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="3"><path d="M14 5l7 7-7 7M3 12h18"/></svg>
          </button>
        </div>

        <!-- Prompt library -->
        <div class="p-8 border-b border-border-primary h-[380px]">
          <h3 class="text-[11px] font-bold text-white uppercase tracking-[0.2em] mb-4">{{ $t('sessionDashboard.prompts') }}</h3>
          <PromptLibrary :session-id="session.id" @inject="copyPrompt" />
        </div>
      </template>

      <!-- ── MEMORY TAB ─────────────────────────────────────────────────── -->
      <template v-else-if="activeTab === 'memory'">
        <div class="p-8 min-h-[300px]">
          <div class="flex items-center justify-between mb-5">
            <h3 class="text-[11px] font-bold text-white uppercase tracking-[0.2em]">Session Memory</h3>
            <button @click="fetchMemory" class="text-[9px] font-mono text-text-tertiary hover:text-text-secondary transition-colors">↻ Refresh</button>
          </div>

          <div v-if="memoryLoading" class="text-[11px] text-text-tertiary font-mono text-center py-12">Loading…</div>
          <div v-else-if="memory.length === 0" class="text-center py-12">
            <p class="text-[11px] text-text-tertiary font-mono opacity-50">No session memory yet</p>
            <p class="text-[10px] text-text-tertiary/40 font-mono mt-1">Agents write here via <code class="bg-white/5 px-1">set_memory</code></p>
          </div>
          <div v-else class="space-y-2">
            <div
              v-for="row in memory"
              :key="row.key"
              class="group border border-border-primary bg-black/20 p-4 rounded"
            >
              <div class="flex items-start justify-between gap-4">
                <div class="flex-1 min-w-0">
                  <span class="text-[10px] font-black text-accent-green uppercase tracking-widest block mb-1">{{ row.key }}</span>
                  <p class="text-[11px] text-text-secondary font-mono leading-relaxed break-words whitespace-pre-wrap">{{ row.value }}</p>
                  <span class="text-[9px] text-text-tertiary/40 font-mono mt-1 block">{{ new Date(row.updated_at).toLocaleString() }}</span>
                </div>
                <button
                  @click="deleteMemoryKey(row.key)"
                  class="opacity-0 group-hover:opacity-100 text-text-tertiary hover:text-accent-red transition-all flex-none mt-0.5"
                  title="Delete"
                >
                  <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M6 18L18 6M6 6l12 12"/></svg>
                </button>
              </div>
            </div>
          </div>
        </div>
      </template>

      <!-- ── DOCUMENTS TAB ──────────────────────────────────────────────── -->
      <template v-else-if="activeTab === 'documents'">
        <div class="p-8 min-h-[300px]">
          <div class="flex items-center justify-between mb-5">
            <h3 class="text-[11px] font-bold text-white uppercase tracking-[0.2em]">Session Documents</h3>
            <button @click="fetchDocuments" class="text-[9px] font-mono text-text-tertiary hover:text-text-secondary transition-colors">↻ Refresh</button>
          </div>

          <div v-if="docsLoading" class="text-[11px] text-text-tertiary font-mono text-center py-12">Loading…</div>
          <div v-else-if="documents.length === 0" class="text-center py-12">
            <p class="text-[11px] text-text-tertiary font-mono opacity-50">No session documents yet</p>
            <p class="text-[10px] text-text-tertiary/40 font-mono mt-1">Agents write here via <code class="bg-white/5 px-1">save_document</code></p>
          </div>
          <div v-else class="space-y-2">
            <div
              v-for="doc in documents"
              :key="doc.id"
              class="group border border-border-primary bg-black/20 rounded overflow-hidden"
            >
              <div
                class="flex items-center gap-3 px-4 py-3 cursor-pointer hover:bg-white/5 transition-colors"
                @click="expandedDoc = expandedDoc === doc.id ? null : doc.id"
              >
                <span class="text-[9px] px-1.5 py-0.5 bg-accent-blue/20 text-accent-blue font-bold uppercase rounded">{{ doc.kind }}</span>
                <span class="text-[11px] font-bold text-text-primary flex-1 truncate">{{ doc.title }}</span>
                <span class="text-[9px] text-text-tertiary/40 font-mono">{{ new Date(doc.updated_at).toLocaleString() }}</span>
                <button
                  @click.stop="deleteDocument(doc.id)"
                  class="opacity-0 group-hover:opacity-100 text-text-tertiary hover:text-accent-red transition-all flex-none"
                  title="Delete"
                >
                  <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M6 18L18 6M6 6l12 12"/></svg>
                </button>
                <svg class="w-3 h-3 text-text-tertiary flex-none transition-transform" :class="expandedDoc === doc.id ? 'rotate-180' : ''" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M19 9l-7 7-7-7"/></svg>
              </div>
              <div v-if="expandedDoc === doc.id" class="border-t border-border-primary/50 bg-black/30 p-4">
                <pre class="text-[11px] text-text-secondary font-mono leading-relaxed whitespace-pre-wrap break-words max-h-80 overflow-y-auto">{{ doc.content || '(empty)' }}</pre>
              </div>
            </div>
          </div>
        </div>
      </template>

      <!-- ── TASKS TAB ──────────────────────────────────────────────────── -->
      <template v-else-if="activeTab === 'tasks'">
        <div class="p-8 min-h-[300px]">
          <div class="flex items-center justify-between mb-5">
            <h3 class="text-[11px] font-bold text-white uppercase tracking-[0.2em]">Session Tasks</h3>
            <button @click="fetchTasks" class="text-[9px] font-mono text-text-tertiary hover:text-text-secondary transition-colors">↻ Refresh</button>
          </div>

          <div v-if="tasksLoading" class="text-[11px] text-text-tertiary font-mono text-center py-12">Loading…</div>
          <div v-else-if="sessionTasks.length === 0" class="text-center py-12">
            <p class="text-[11px] text-text-tertiary font-mono opacity-50">No session tasks yet</p>
            <p class="text-[10px] text-text-tertiary/40 font-mono mt-1">Agents create tasks via <code class="bg-white/5 px-1">create_task</code></p>
          </div>
          <div v-else class="space-y-2">
            <div
              v-for="task in sessionTasks"
              :key="task.id"
              class="border border-border-primary bg-black/20 p-4 rounded flex items-start gap-4"
            >
              <div class="flex-1 min-w-0">
                <p class="text-[12px] font-bold text-text-primary mb-2">{{ task.title }}</p>
                <div class="flex items-center gap-2 flex-wrap">
                  <span class="text-[9px] px-1.5 py-0.5 rounded font-bold uppercase" :class="statusClass[task.status] ?? 'bg-text-tertiary/20 text-text-tertiary'">{{ task.status }}</span>
                  <span class="text-[9px] px-1.5 py-0.5 rounded font-bold uppercase" :class="priorityClass[task.priority] ?? 'bg-text-tertiary/20 text-text-tertiary'">{{ task.priority }}</span>
                  <span v-if="task.due_date" class="text-[9px] font-mono text-accent-yellow/70">due {{ task.due_date }}</span>
                  <span v-if="task.assigned_to" class="text-[9px] font-mono text-text-tertiary/60">→ {{ task.assigned_to }}</span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </template>

      <!-- ── TIMELINE TAB ───────────────────────────────────────────────── -->
      <template v-else-if="activeTab === 'timeline'">
        <div class="p-8 h-[500px]">
          <GanttView :session-id="session.id" :project-color="'var(--color-accent-green)'" />
        </div>
      </template>

      <!-- ── REMINDERS TAB ──────────────────────────────────────────────── -->
      <template v-else-if="activeTab === 'reminders'">
        <div class="p-8 min-h-[300px]">
          <div class="flex items-center justify-between mb-5">
            <h3 class="text-[11px] font-bold text-white uppercase tracking-[0.2em]">Session Reminders</h3>
            <button @click="fetchReminders" class="text-[9px] font-mono text-text-tertiary hover:text-text-secondary transition-colors">↻ Refresh</button>
          </div>

          <div v-if="remindersLoading" class="text-[11px] text-text-tertiary font-mono text-center py-12">Loading…</div>
          <div v-else-if="sessionReminders.length === 0" class="text-center py-12">
            <p class="text-[11px] text-text-tertiary font-mono opacity-50">No session reminders yet</p>
            <p class="text-[10px] text-text-tertiary/40 font-mono mt-1">Agents create reminders via <code class="bg-white/5 px-1">create_reminder</code></p>
          </div>
          <div v-else class="space-y-2">
            <div
              v-for="reminder in sessionReminders"
              :key="reminder.id"
              class="border border-border-primary bg-black/20 p-4 rounded flex items-center gap-4"
            >
              <div class="flex-1 min-w-0">
                <p class="text-[12px] font-bold text-text-primary mb-1">{{ reminder.title }}</p>
                <div class="flex items-center gap-2">
                  <span class="text-[9px] px-1.5 py-0.5 rounded font-bold uppercase" :class="statusClass[reminder.status] ?? 'bg-text-tertiary/20 text-text-tertiary'">{{ reminder.status }}</span>
                  <span class="text-[9px] font-mono text-accent-yellow/70">due {{ new Date(reminder.due_at).toLocaleString() }}</span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </template>

      <!-- Footer -->
      <div class="px-8 py-4 bg-black/40 flex justify-between items-center border-t border-white/5 text-[9px]">
        <span class="text-text-tertiary font-bold uppercase tracking-[0.4em] opacity-40">{{ $t('sessionDashboard.footer') }}</span>
        <span class="text-text-tertiary/20 font-mono italic">PROJECT_REF: {{ session.projectId }}</span>
      </div>
    </div>
  </div>
</template>
