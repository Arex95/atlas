<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import { api } from '@/api/client';

const props = defineProps<{
  projectId?: string;
  sessionId?: string;
  projectColor?: string;
}>();

interface RawTask {
  id: string;
  title: string;
  status: string;
  priority: string;
  dueDate?: string;
  due_date?: string;
  createdAt?: string;
  created_at?: string;
}

interface Task {
  id: string;
  title: string;
  status: string;
  priority: string;
  dueDate?: string;
  createdAt: string;
}

const tasks = ref<Task[]>([]);
const loading = ref(false);

function normalizeTask(t: RawTask): Task {
  return {
    id: t.id,
    title: t.title,
    status: t.status,
    priority: t.priority,
    dueDate: t.dueDate ?? t.due_date,
    createdAt: t.createdAt ?? t.created_at ?? new Date().toISOString(),
  };
}

async function fetchTasks() {
  loading.value = true;
  try {
    const url = props.sessionId
      ? `/api/sessions/${props.sessionId}/tasks`
      : `/api/tasks?projectId=${props.projectId}`;
    const raw = await api.get<RawTask[]>(url);
    tasks.value = raw.map(normalizeTask);
  } catch {
    tasks.value = [];
  } finally {
    loading.value = false;
  }
}

onMounted(fetchTasks);

// Build a timeline spanning from min(createdAt) to max(dueDate or today)
const today = new Date();

function parseDate(s: string): Date {
  return new Date(s.slice(0, 10));
}

const timelineStart = computed<Date>(() => {
  if (!tasks.value.length) return today;
  return tasks.value.reduce<Date>((min, t) => {
    const d = parseDate(t.createdAt);
    return d < min ? d : min;
  }, parseDate(tasks.value[0].createdAt));
});

const timelineEnd = computed<Date>(() => {
  if (!tasks.value.length) return today;
  return tasks.value.reduce<Date>((max, t) => {
    const d = t.dueDate ? parseDate(t.dueDate) : today;
    return d > max ? d : max;
  }, today);
});

const totalDays = computed(() => {
  const ms = timelineEnd.value.getTime() - timelineStart.value.getTime();
  return Math.max(Math.ceil(ms / 86400000) + 2, 14);
});

function dayOffset(dateStr: string): number {
  const d = parseDate(dateStr);
  return Math.max(0, Math.ceil((d.getTime() - timelineStart.value.getTime()) / 86400000));
}

function barLeft(task: Task): string {
  const offset = dayOffset(task.createdAt);
  return `${(offset / totalDays.value) * 100}%`;
}

function barWidth(task: Task): string {
  if (!task.dueDate) return `${(3 / totalDays.value) * 100}%`;
  const start = dayOffset(task.createdAt);
  const end = dayOffset(task.dueDate);
  const span = Math.max(end - start, 1);
  return `${(span / totalDays.value) * 100}%`;
}

function barColor(task: Task): string {
  if (task.status === 'done') return 'var(--color-accent-green)';
  if (task.status === 'blocked') return 'var(--color-accent-red)';
  if (task.priority === 'critical') return 'var(--color-accent-orange)';
  if (task.priority === 'high') return 'var(--color-accent-yellow)';
  return props.projectColor || 'var(--color-accent-blue)';
}

function priorityLabel(p: string): string {
  const map: Record<string, string> = { critical: '!!!', high: '!!', medium: '!', low: '·' };
  return map[p] || '·';
}

// Month tick marks on the timeline
const monthTicks = computed(() => {
  const ticks: { label: string; offset: number }[] = [];
  const cur = new Date(timelineStart.value);
  cur.setDate(1);
  while (cur <= timelineEnd.value) {
    const offset = Math.ceil((cur.getTime() - timelineStart.value.getTime()) / 86400000);
    ticks.push({
      label: cur.toLocaleString('default', { month: 'short', year: '2-digit' }),
      offset: (offset / totalDays.value) * 100,
    });
    cur.setMonth(cur.getMonth() + 1);
  }
  return ticks;
});

const todayOffset = computed(() => {
  const offset = Math.ceil((today.getTime() - timelineStart.value.getTime()) / 86400000);
  return (offset / totalDays.value) * 100;
});

const statusBg: Record<string, string> = {
  todo: 'bg-text-tertiary/20 text-text-tertiary',
  'in-progress': 'bg-accent-blue/20 text-accent-blue',
  done: 'bg-accent-green/20 text-accent-green',
  blocked: 'bg-accent-red/20 text-accent-red',
};
</script>

<template>
  <div class="h-full overflow-y-auto">
    <div v-if="loading" class="flex items-center justify-center h-40 text-text-tertiary text-[11px] font-mono">
      {{ $t('gantt.loading') }}
    </div>

    <div v-else-if="tasks.length === 0" class="flex flex-col items-center justify-center h-40 opacity-40">
      <p class="text-[11px] text-text-tertiary uppercase tracking-widest font-bold">{{ $t('gantt.noTasks') }}</p>
      <p class="text-[10px] text-text-tertiary opacity-60 mt-1">{{ $t('gantt.noTasksHint') }}</p>
    </div>

    <div v-else class="p-4 min-w-0">
      <!-- Header: month ticks -->
      <div class="relative h-6 mb-2 ml-40">
        <div
          v-for="tick in monthTicks"
          :key="tick.label"
          class="absolute top-0 text-[9px] font-mono text-text-tertiary/60"
          :style="{ left: tick.offset + '%' }"
        >{{ tick.label }}</div>
        <!-- Today marker label -->
        <div
          class="absolute top-0 text-[9px] font-mono text-accent-blue/80 font-bold"
          :style="{ left: todayOffset + '%' }"
        >▼</div>
      </div>

      <!-- Task rows -->
      <div class="space-y-1.5">
        <div v-for="task in tasks" :key="task.id" class="flex items-center gap-2 min-w-0">
          <!-- Task label -->
          <div class="flex-none w-40 flex items-center gap-2 pr-2">
            <span class="text-[9px] font-mono" :style="{ color: barColor(task) }">{{ priorityLabel(task.priority) }}</span>
            <div class="flex-1 min-w-0">
              <p class="text-[10px] font-mono text-text-primary truncate" :class="task.status === 'done' ? 'line-through opacity-50' : ''">{{ task.title }}</p>
              <span class="text-[8px] px-1 py-0.5 rounded font-bold uppercase" :class="statusBg[task.status] || 'bg-text-tertiary/20 text-text-tertiary'">{{ task.status }}</span>
            </div>
          </div>

          <!-- Bar track -->
          <div class="flex-1 relative h-5 bg-bg-sidebar/30 rounded overflow-hidden min-w-0">
            <!-- Today line -->
            <div
              class="absolute top-0 bottom-0 w-px bg-accent-blue/40 z-10"
              :style="{ left: todayOffset + '%' }"
            />
            <!-- Task bar -->
            <div
              class="absolute top-1 bottom-1 rounded-sm opacity-80 min-w-[4px] transition-all"
              :style="{
                left: barLeft(task),
                width: barWidth(task),
                backgroundColor: barColor(task),
              }"
              :title="task.title + (task.dueDate ? ' → ' + task.dueDate : '')"
            />
          </div>

          <!-- Due date -->
          <div class="flex-none w-20 text-right">
            <span v-if="task.dueDate" class="text-[9px] font-mono text-text-tertiary">{{ task.dueDate }}</span>
            <span v-else class="text-[9px] font-mono text-text-tertiary/30">{{ $t('gantt.noDate') }}</span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
