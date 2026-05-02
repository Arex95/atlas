<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue';
import { api } from '@/api/client';

const props = defineProps<{ projectId: string }>();

interface Task {
  id: string;
  projectId: string;
  title: string;
  description: string;
  status: string;
  priority: string;
  dueDate: string | null;
  assignedTo: string | null;
  tags: string[];
  parentId: string | null;
  createdAt: string;
  updatedAt: string;
}

const tasks = ref<Task[]>([]);
const loading = ref(false);
const filterStatus = ref<string>('all');
const showForm = ref(false);
const editingId = ref<string | null>(null);
const expandedSubtasks = ref<Set<string>>(new Set());
const subtasksCache = ref<Record<string, Task[]>>({});

const form = ref({
  title: '',
  description: '',
  status: 'todo',
  priority: 'medium',
  dueDate: '',
  assignedTo: '',
  tags: [] as string[],
  parentId: '',
  tagInput: '',
});
const saving = ref(false);

async function load() {
  loading.value = true;
  try {
    tasks.value = await api.get<Task[]>(`/api/tasks?projectId=${props.projectId}`);
    expandedSubtasks.value = new Set();
    subtasksCache.value = {};
  } catch {
    tasks.value = [];
  } finally {
    loading.value = false;
  }
}

onMounted(load);
watch(() => props.projectId, load);

const rootTasks = computed(() => tasks.value.filter(t => !t.parentId));

const subtaskCounts = computed(() => {
  const map: Record<string, number> = {};
  for (const t of tasks.value) {
    if (t.parentId) map[t.parentId] = (map[t.parentId] ?? 0) + 1;
  }
  return map;
});

const rootTaskOptions = computed(() =>
  rootTasks.value.filter(t => t.id !== editingId.value)
);

const filtered = computed(() => {
  const roots = rootTasks.value;
  if (filterStatus.value === 'all') return roots;
  return roots.filter(t => t.status === filterStatus.value);
});

const counts = computed(() => ({
  all: rootTasks.value.length,
  todo: rootTasks.value.filter(t => t.status === 'todo').length,
  'in-progress': rootTasks.value.filter(t => t.status === 'in-progress').length,
  done: rootTasks.value.filter(t => t.status === 'done').length,
  blocked: rootTasks.value.filter(t => t.status === 'blocked').length,
}));

function startCreate() {
  editingId.value = null;
  form.value = { title: '', description: '', status: 'todo', priority: 'medium', dueDate: '', assignedTo: '', tags: [], parentId: '', tagInput: '' };
  showForm.value = true;
}

function startEdit(task: Task) {
  editingId.value = task.id;
  form.value = {
    title: task.title,
    description: task.description,
    status: task.status,
    priority: task.priority,
    dueDate: task.dueDate || '',
    assignedTo: task.assignedTo || '',
    tags: [...task.tags],
    parentId: task.parentId || '',
    tagInput: '',
  };
  showForm.value = true;
}

function cancelForm() {
  showForm.value = false;
  editingId.value = null;
}

function addTag() {
  const raw = form.value.tagInput.trim().replace(/,$/, '');
  if (raw && !form.value.tags.includes(raw)) {
    form.value.tags.push(raw);
  }
  form.value.tagInput = '';
}

function removeTag(tag: string) {
  form.value.tags = form.value.tags.filter(t => t !== tag);
}

function onTagKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' || e.key === ',') {
    e.preventDefault();
    addTag();
  } else if (e.key === 'Backspace' && !form.value.tagInput && form.value.tags.length > 0) {
    form.value.tags.pop();
  }
}

async function saveForm() {
  if (!form.value.title.trim()) return;
  saving.value = true;
  try {
    const payload: Record<string, unknown> = {
      title: form.value.title.trim(),
      description: form.value.description.trim(),
      status: form.value.status,
      priority: form.value.priority,
      dueDate: form.value.dueDate || null,
      assignedTo: form.value.assignedTo.trim() || null,
      projectId: props.projectId,
      tags: form.value.tags,
      parentId: form.value.parentId || null,
    };
    if (editingId.value) {
      await api.patch(`/api/tasks/${editingId.value}`, payload);
    } else {
      await api.post('/api/tasks', payload);
    }
    showForm.value = false;
    editingId.value = null;
    await load();
  } finally {
    saving.value = false;
  }
}

async function cycleStatus(task: Task) {
  const cycle: Record<string, string> = { todo: 'in-progress', 'in-progress': 'done', done: 'todo', blocked: 'todo' };
  const next = cycle[task.status] || 'todo';
  await api.patch(`/api/tasks/${task.id}`, { status: next });
  task.status = next;
  if (task.parentId && subtasksCache.value[task.parentId]) {
    const sub = subtasksCache.value[task.parentId].find(s => s.id === task.id);
    if (sub) sub.status = next;
  }
}

async function toggleSubtasks(taskId: string) {
  if (expandedSubtasks.value.has(taskId)) {
    expandedSubtasks.value.delete(taskId);
    return;
  }
  expandedSubtasks.value.add(taskId);
  if (subtasksCache.value[taskId]) return;
  try {
    subtasksCache.value[taskId] = await api.get<Task[]>(
      `/api/tasks?projectId=${props.projectId}&parentId=${taskId}`
    );
  } catch {
    subtasksCache.value[taskId] = [];
  }
}

async function remove(id: string, parentId: string | null) {
  await api.delete(`/api/tasks/${id}`);
  if (parentId && subtasksCache.value[parentId]) {
    subtasksCache.value[parentId] = subtasksCache.value[parentId].filter(s => s.id !== id);
  } else {
    tasks.value = tasks.value.filter(t => t.id !== id);
    delete subtasksCache.value[id];
  }
}

const statusConfig: Record<string, { label: string; color: string; ring: string }> = {
  'todo':        { label: 'Todo',        color: 'text-text-tertiary',  ring: 'ring-white/20' },
  'in-progress': { label: 'In Progress', color: 'text-accent-blue',    ring: 'ring-accent-blue/50' },
  'done':        { label: 'Done',        color: 'text-accent-green',   ring: 'ring-accent-green/50' },
  'blocked':     { label: 'Blocked',     color: 'text-red-400',        ring: 'ring-red-400/50' },
};

const priorityConfig: Record<string, { label: string; dot: string }> = {
  low:      { label: 'Low',      dot: 'bg-text-tertiary/40' },
  medium:   { label: 'Medium',   dot: 'bg-yellow-400/80' },
  high:     { label: 'High',     dot: 'bg-orange-400' },
  critical: { label: 'Critical', dot: 'bg-red-500' },
};

function isOverdue(task: Task): boolean {
  if (!task.dueDate || task.status === 'done') return false;
  return new Date(task.dueDate) < new Date();
}

function formatDate(d: string): string {
  return new Date(d + 'T12:00:00').toLocaleDateString('en', { month: 'short', day: 'numeric' });
}
</script>

<template>
  <div class="h-full flex flex-col gap-4">

    <!-- Header -->
    <div class="flex items-center justify-between flex-none">
      <div class="flex gap-1">
        <button
          v-for="s in ['all', 'todo', 'in-progress', 'done', 'blocked']"
          :key="s"
          class="px-3 py-1 text-[9px] font-black uppercase tracking-widest rounded transition-colors"
          :class="filterStatus === s
            ? 'bg-accent-blue/20 text-accent-blue'
            : 'text-text-tertiary hover:text-text-secondary'"
          @click="filterStatus = s"
        >
          {{ s === 'all' ? $t('projectTasks.filterAll') : s }}
          <span class="ml-1 opacity-60">{{ counts[s as keyof typeof counts] }}</span>
        </button>
      </div>
      <button
        class="flex items-center gap-1.5 px-3 py-1.5 text-[9px] font-black uppercase tracking-wider bg-accent-blue/20 text-accent-blue hover:bg-accent-blue/30 rounded transition-colors"
        @click="startCreate"
      >
        <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="3"><path d="M12 5v14M5 12h14"/></svg>
        {{ $t('projectTasks.newTask') }}
      </button>
    </div>

    <!-- Form -->
    <div v-if="showForm" class="border border-accent-blue/30 bg-accent-blue/5 rounded-lg p-4 space-y-3 flex-none">
      <input
        v-model="form.title"
        class="w-full bg-black/30 border border-white/10 rounded px-3 py-1.5 text-[12px] text-white placeholder-text-tertiary focus:outline-none focus:border-accent-blue"
        :placeholder="$t('projectTasks.taskTitlePlaceholder')"
        autofocus
        @keyup.enter="saveForm"
        @keyup.esc="cancelForm"
      />
      <textarea
        v-model="form.description"
        class="w-full bg-black/30 border border-white/10 rounded px-3 py-1.5 text-[11px] text-text-secondary placeholder-text-tertiary focus:outline-none focus:border-accent-blue resize-none h-16"
        :placeholder="$t('projectTasks.taskDescriptionPlaceholder')"
      />
      <div class="grid grid-cols-4 gap-2">
        <div class="space-y-1">
          <label class="text-[8px] font-black uppercase tracking-widest text-text-tertiary opacity-50">{{ $t('projectTasks.statusLabel') }}</label>
          <select v-model="form.status" class="w-full bg-black/30 border border-white/10 rounded px-2 py-1 text-[11px] text-text-primary focus:outline-none focus:border-accent-blue">
            <option value="todo">{{ $t('projectTasks.statusTodo') }}</option>
            <option value="in-progress">{{ $t('projectTasks.statusInProgress') }}</option>
            <option value="done">{{ $t('projectTasks.statusDone') }}</option>
            <option value="blocked">{{ $t('projectTasks.statusBlocked') }}</option>
          </select>
        </div>
        <div class="space-y-1">
          <label class="text-[8px] font-black uppercase tracking-widest text-text-tertiary opacity-50">{{ $t('projectTasks.priorityLabel') }}</label>
          <select v-model="form.priority" class="w-full bg-black/30 border border-white/10 rounded px-2 py-1 text-[11px] text-text-primary focus:outline-none focus:border-accent-blue">
            <option value="low">{{ $t('projectTasks.priorityLow') }}</option>
            <option value="medium">{{ $t('projectTasks.priorityMedium') }}</option>
            <option value="high">{{ $t('projectTasks.priorityHigh') }}</option>
            <option value="critical">{{ $t('projectTasks.priorityCritical') }}</option>
          </select>
        </div>
        <div class="space-y-1">
          <label class="text-[8px] font-black uppercase tracking-widest text-text-tertiary opacity-50">{{ $t('projectTasks.dueDateLabel') }}</label>
          <input v-model="form.dueDate" type="date" class="w-full bg-black/30 border border-white/10 rounded px-2 py-1 text-[11px] text-text-primary focus:outline-none focus:border-accent-blue" />
        </div>
        <div class="space-y-1">
          <label class="text-[8px] font-black uppercase tracking-widest text-text-tertiary opacity-50">{{ $t('projectTasks.assignedToLabel') }}</label>
          <input v-model="form.assignedTo" class="w-full bg-black/30 border border-white/10 rounded px-2 py-1 text-[11px] text-text-primary placeholder-text-tertiary focus:outline-none focus:border-accent-blue" :placeholder="$t('projectTasks.assignedToPlaceholder')" />
        </div>
      </div>
      <div class="grid grid-cols-2 gap-2">
        <div class="space-y-1">
          <label class="text-[8px] font-black uppercase tracking-widest text-text-tertiary opacity-50">{{ $t('projectTasks.parentTaskLabel') }}</label>
          <select v-model="form.parentId" class="w-full bg-black/30 border border-white/10 rounded px-2 py-1 text-[11px] text-text-primary focus:outline-none focus:border-accent-blue">
            <option value="">{{ $t('projectTasks.parentTaskNone') }}</option>
            <option v-for="t in rootTaskOptions" :key="t.id" :value="t.id">{{ t.title }}</option>
          </select>
        </div>
        <div class="space-y-1">
          <label class="text-[8px] font-black uppercase tracking-widest text-text-tertiary opacity-50">{{ $t('projectTasks.tagsLabel') }}</label>
          <div class="flex flex-wrap items-center gap-1 bg-black/30 border border-white/10 rounded px-2 py-1 min-h-[28px] focus-within:border-accent-blue">
            <span
              v-for="tag in form.tags"
              :key="tag"
              class="flex items-center gap-0.5 bg-white/5 text-text-tertiary text-[9px] px-1.5 py-0.5 rounded cursor-pointer hover:bg-red-500/20 hover:text-red-400 transition-colors"
              @click="removeTag(tag)"
            >{{ tag }}<span class="opacity-50 ml-0.5">×</span></span>
            <input
              v-model="form.tagInput"
              class="flex-1 min-w-[60px] bg-transparent text-[11px] text-text-primary placeholder-text-tertiary focus:outline-none"
              :placeholder="$t('projectTasks.tagsPlaceholder')"
              @keydown="onTagKeydown"
              @blur="addTag"
            />
          </div>
        </div>
      </div>
      <div class="flex gap-2 pt-1">
        <button
          class="px-4 py-1.5 text-[9px] font-black uppercase tracking-wider bg-accent-blue/20 text-accent-blue hover:bg-accent-blue/30 rounded transition-colors disabled:opacity-40"
          :disabled="saving || !form.title.trim()"
          @click="saveForm"
        >
          {{ saving ? $t('projectTasks.saving') : (editingId ? $t('projectTasks.update') : $t('projectTasks.create')) }}
        </button>
        <button class="px-4 py-1.5 text-[9px] font-black uppercase tracking-wider text-text-tertiary hover:text-text-primary transition-colors" @click="cancelForm">
          {{ $t('projectTasks.cancel') }}
        </button>
      </div>
    </div>

    <!-- Task list -->
    <div class="flex-1 overflow-y-auto space-y-1 pr-1">
      <div v-if="loading" class="flex items-center justify-center py-12 opacity-30">
        <div class="w-4 h-4 border border-accent-blue border-t-transparent rounded-full animate-spin"/>
      </div>

      <template v-else-if="filtered.length > 0">
        <template v-for="task in filtered" :key="task.id">
          <div
            class="group flex items-start gap-3 p-3 bg-black/20 border border-white/5 rounded-lg hover:border-white/10 hover:bg-black/30 transition-all"
          >
            <!-- Status toggle circle -->
            <button
              class="mt-0.5 w-4 h-4 rounded-full border flex-none ring-2 ring-offset-1 ring-offset-bg-primary transition-all hover:scale-110"
              :class="[statusConfig[task.status]?.ring || 'ring-white/20', task.status === 'done' ? 'bg-accent-green border-accent-green' : 'bg-transparent border-white/20']"
              :title="`Click to cycle: ${task.status}`"
              @click="cycleStatus(task)"
            >
              <svg v-if="task.status === 'done'" class="w-full h-full text-white p-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="3"><path d="M5 13l4 4L19 7"/></svg>
              <svg v-else-if="task.status === 'in-progress'" class="w-full h-full text-accent-blue p-0.5" fill="currentColor" viewBox="0 0 24 24"><circle cx="12" cy="12" r="5"/></svg>
              <svg v-else-if="task.status === 'blocked'" class="w-full h-full text-red-400 p-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="3"><path d="M6 18L18 6M6 6l12 12"/></svg>
            </button>

            <div class="flex-1 min-w-0">
              <p class="text-[12px] font-bold text-text-primary leading-tight truncate" :class="task.status === 'done' ? 'line-through opacity-40' : ''">
                {{ task.title }}
              </p>
              <p v-if="task.description" class="text-[10px] text-text-tertiary mt-0.5 opacity-60 truncate">{{ task.description }}</p>
              <div class="flex items-center flex-wrap gap-3 mt-1.5">
                <!-- Priority dot -->
                <div class="flex items-center gap-1">
                  <span class="w-1.5 h-1.5 rounded-full" :class="priorityConfig[task.priority]?.dot || 'bg-text-tertiary/40'" />
                  <span class="text-[8px] font-bold uppercase tracking-widest text-text-tertiary opacity-50">{{ task.priority }}</span>
                </div>
                <!-- Due date -->
                <div v-if="task.dueDate" class="flex items-center gap-1" :class="isOverdue(task) ? 'text-red-400' : 'text-text-tertiary opacity-50'">
                  <svg class="w-2.5 h-2.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/></svg>
                  <span class="text-[8px] font-mono font-bold">{{ formatDate(task.dueDate) }}</span>
                  <span v-if="isOverdue(task)" class="text-[7px] font-black uppercase">{{ $t('projectTasks.overdue') }}</span>
                </div>
                <!-- Assigned -->
                <span v-if="task.assignedTo" class="text-[8px] font-mono text-text-tertiary opacity-40">@{{ task.assignedTo }}</span>
                <!-- Subtask counter -->
                <button
                  v-if="subtaskCounts[task.id]"
                  class="flex items-center gap-1 text-[8px] font-bold text-text-tertiary opacity-50 hover:opacity-100 hover:text-accent-blue transition-all"
                  @click="toggleSubtasks(task.id)"
                >
                  <span>{{ expandedSubtasks.has(task.id) ? '▾' : '▸' }}</span>
                  <span>{{ subtaskCounts[task.id] }} {{ subtaskCounts[task.id] === 1 ? $t('projectTasks.subtask') : $t('projectTasks.subtasks') }}</span>
                </button>
              </div>
              <!-- Tags -->
              <div v-if="task.tags.length > 0" class="flex flex-wrap gap-1 mt-1.5">
                <span
                  v-for="tag in task.tags"
                  :key="tag"
                  class="bg-white/5 text-text-tertiary text-[9px] px-1.5 py-0.5 rounded"
                >{{ tag }}</span>
              </div>
            </div>

            <!-- Actions -->
            <div class="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity flex-none">
              <button class="p-1 rounded hover:bg-white/5 text-text-tertiary hover:text-text-primary transition-colors" @click="startEdit(task)">
                <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z"/></svg>
              </button>
              <button class="p-1 rounded hover:bg-red-500/10 text-text-tertiary hover:text-red-400 transition-colors" @click="remove(task.id, null)">
                <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"/></svg>
              </button>
            </div>
          </div>

          <!-- Subtasks (lazy-loaded on toggle) -->
          <template v-if="expandedSubtasks.has(task.id)">
            <div
              v-for="sub in subtasksCache[task.id] ?? []"
              :key="sub.id"
              class="group ml-6 border-l border-border-primary pl-3 flex items-start gap-3 p-3 bg-black/10 border-y border-white/5 hover:bg-black/20 transition-all"
            >
              <button
                class="mt-0.5 w-3.5 h-3.5 rounded-full border flex-none ring-2 ring-offset-1 ring-offset-bg-primary transition-all hover:scale-110"
                :class="[statusConfig[sub.status]?.ring || 'ring-white/20', sub.status === 'done' ? 'bg-accent-green border-accent-green' : 'bg-transparent border-white/20']"
                @click="cycleStatus(sub)"
              >
                <svg v-if="sub.status === 'done'" class="w-full h-full text-white p-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="3"><path d="M5 13l4 4L19 7"/></svg>
                <svg v-else-if="sub.status === 'in-progress'" class="w-full h-full text-accent-blue p-0.5" fill="currentColor" viewBox="0 0 24 24"><circle cx="12" cy="12" r="5"/></svg>
                <svg v-else-if="sub.status === 'blocked'" class="w-full h-full text-red-400 p-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="3"><path d="M6 18L18 6M6 6l12 12"/></svg>
              </button>
              <div class="flex-1 min-w-0">
                <p class="text-[11px] font-semibold text-text-secondary leading-tight truncate" :class="sub.status === 'done' ? 'line-through opacity-40' : ''">
                  {{ sub.title }}
                </p>
                <div v-if="sub.tags.length > 0" class="flex flex-wrap gap-1 mt-1">
                  <span
                    v-for="tag in sub.tags"
                    :key="tag"
                    class="bg-white/5 text-text-tertiary text-[9px] px-1.5 py-0.5 rounded"
                  >{{ tag }}</span>
                </div>
              </div>
              <div class="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity flex-none">
                <button class="p-1 rounded hover:bg-white/5 text-text-tertiary hover:text-text-primary transition-colors" @click="startEdit(sub)">
                  <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z"/></svg>
                </button>
                <button class="p-1 rounded hover:bg-red-500/10 text-text-tertiary hover:text-red-400 transition-colors" @click="remove(sub.id, task.id)">
                  <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"/></svg>
                </button>
              </div>
            </div>
          </template>
        </template>
      </template>

      <div v-else class="flex flex-col items-center justify-center py-12 opacity-25 gap-2">
        <svg class="w-8 h-8 text-text-tertiary" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1">
          <path d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2"/>
        </svg>
        <p class="text-[10px] text-text-tertiary uppercase tracking-widest font-bold">{{ $t('projectTasks.noTasks') }}</p>
      </div>
    </div>
  </div>
</template>
