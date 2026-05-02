<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import type { Project } from '@atlas/domain';
import { api } from '@/api/client';

const props = defineProps<{ projects: Project[] }>();

interface Reminder {
  id: string;
  projectId: string;
  title: string;
  dueAt: string;
  status: string;
}

interface Task {
  id: string;
  projectId: string;
  title: string;
  status: string;
  priority: string;
  dueDate: string | null;
}

interface CalendarEvent {
  id: string;
  title: string;
  date: string;
  type: 'deadline' | 'reminder' | 'task';
  color: string;
  projectName: string;
  priority?: string;
}

const reminders = ref<Reminder[]>([]);
const tasks = ref<Task[]>([]);
const today = new Date();
const currentYear = ref(today.getFullYear());
const currentMonth = ref(today.getMonth());
const selectedDay = ref<number | null>(today.getDate());

async function fetchData() {
  try {
    [reminders.value, tasks.value] = await Promise.all([
      api.get<Reminder[]>('/api/reminders'),
      api.get<Task[]>('/api/tasks'),
    ]);
  } catch {
    reminders.value = [];
    tasks.value = [];
  }
}

onMounted(fetchData);

function prevMonth() {
  if (currentMonth.value === 0) { currentMonth.value = 11; currentYear.value--; }
  else currentMonth.value--;
  selectedDay.value = null;
}

function nextMonth() {
  if (currentMonth.value === 11) { currentMonth.value = 0; currentYear.value++; }
  else currentMonth.value++;
  selectedDay.value = null;
}

const monthName = computed(() =>
  new Date(currentYear.value, currentMonth.value, 1).toLocaleString('en', { month: 'long' })
);

const projectMap = computed(() =>
  Object.fromEntries(props.projects.map(p => [p.id, p]))
);

const allEvents = computed((): CalendarEvent[] => {
  const events: CalendarEvent[] = [];

  // Project deadlines
  for (const p of props.projects) {
    if (p.deadline?.date) {
      events.push({
        id: `deadline-${p.id}`,
        title: `${p.name} deadline`,
        date: p.deadline.date.slice(0, 10),
        type: 'deadline',
        color: p.color || '#3b82f6',
        projectName: p.name,
      });
    }
  }

  // Reminders
  for (const r of reminders.value) {
    if (r.status === 'done') continue;
    const proj = projectMap.value[r.projectId];
    events.push({
      id: r.id,
      title: r.title,
      date: r.dueAt.slice(0, 10),
      type: 'reminder',
      color: proj?.color || '#f59e0b',
      projectName: proj?.name || 'General',
    });
  }

  // Tasks with due date (exclude done)
  for (const t of tasks.value) {
    if (!t.dueDate || t.status === 'done') continue;
    const proj = projectMap.value[t.projectId];
    events.push({
      id: `task-${t.id}`,
      title: t.title,
      date: t.dueDate.slice(0, 10),
      type: 'task',
      color: proj?.color || '#6b7280',
      projectName: proj?.name || 'Unknown',
      priority: t.priority,
    });
  }

  return events;
});

// Returns events grouped by YYYY-MM-DD key
const eventsByDate = computed(() => {
  const map: Record<string, CalendarEvent[]> = {};
  for (const ev of allEvents.value) {
    if (!map[ev.date]) map[ev.date] = [];
    map[ev.date].push(ev);
  }
  return map;
});

function dateKey(year: number, month: number, day: number): string {
  return `${year}-${String(month + 1).padStart(2, '0')}-${String(day).padStart(2, '0')}`;
}

// Calendar grid: 6 rows × 7 cols, starting Monday
const calendarDays = computed(() => {
  const firstDay = new Date(currentYear.value, currentMonth.value, 1).getDay();
  // Convert Sunday=0 to Monday=0 offset
  const offset = (firstDay + 6) % 7;
  const daysInMonth = new Date(currentYear.value, currentMonth.value + 1, 0).getDate();
  const daysInPrev = new Date(currentYear.value, currentMonth.value, 0).getDate();
  const cells: { day: number; month: 'prev' | 'current' | 'next'; key: string }[] = [];

  for (let i = offset - 1; i >= 0; i--) {
    const d = daysInPrev - i;
    const m = currentMonth.value === 0 ? 11 : currentMonth.value - 1;
    const y = currentMonth.value === 0 ? currentYear.value - 1 : currentYear.value;
    cells.push({ day: d, month: 'prev', key: dateKey(y, m, d) });
  }
  for (let d = 1; d <= daysInMonth; d++) {
    cells.push({ day: d, month: 'current', key: dateKey(currentYear.value, currentMonth.value, d) });
  }
  let next = 1;
  while (cells.length < 42) {
    const m = currentMonth.value === 11 ? 0 : currentMonth.value + 1;
    const y = currentMonth.value === 11 ? currentYear.value + 1 : currentYear.value;
    cells.push({ day: next, month: 'next', key: dateKey(y, m, next) });
    next++;
  }
  return cells;
});

const todayKey = computed(() => dateKey(today.getFullYear(), today.getMonth(), today.getDate()));

const selectedDayKey = computed(() =>
  selectedDay.value !== null ? dateKey(currentYear.value, currentMonth.value, selectedDay.value) : null
);

const selectedEvents = computed(() =>
  selectedDayKey.value ? (eventsByDate.value[selectedDayKey.value] || []) : []
);

const weekdays = ['calendar.weekdayMon', 'calendar.weekdayTue', 'calendar.weekdayWed', 'calendar.weekdayThu', 'calendar.weekdayFri', 'calendar.weekdaySat', 'calendar.weekdaySun'];
</script>

<template>
  <div class="bg-bg-sidebar/30 border border-border-primary rounded-lg overflow-hidden">

    <!-- Header -->
    <div class="flex items-center justify-between px-5 py-3 border-b border-border-primary bg-black/20">
      <div class="flex items-center gap-3">
        <svg class="w-3.5 h-3.5 text-accent-blue" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <rect x="3" y="4" width="18" height="18" rx="2" ry="2"/><line x1="16" y1="2" x2="16" y2="6"/><line x1="8" y1="2" x2="8" y2="6"/><line x1="3" y1="10" x2="21" y2="10"/>
        </svg>
        <span class="text-[10px] font-black uppercase tracking-[0.3em] text-text-secondary">
          {{ monthName }} {{ currentYear }}
        </span>
      </div>
      <div class="flex items-center gap-1">
        <button class="p-1 rounded hover:bg-white/5 text-text-tertiary hover:text-white transition-colors" @click="prevMonth">
          <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5"><path d="M15 18l-6-6 6-6"/></svg>
        </button>
        <button class="p-1 rounded hover:bg-white/5 text-text-tertiary hover:text-white transition-colors" @click="nextMonth">
          <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5"><path d="M9 18l6-6-6-6"/></svg>
        </button>
      </div>
    </div>

    <div class="flex gap-0">
      <!-- Calendar grid -->
      <div class="flex-1 p-4">
        <!-- Weekday labels -->
        <div class="grid grid-cols-7 mb-2">
          <div v-for="wd in weekdays" :key="wd" class="text-center text-[8px] font-black uppercase tracking-widest text-text-tertiary opacity-40 py-1">
            {{ $t(wd) }}
          </div>
        </div>

        <!-- Day cells -->
        <div class="grid grid-cols-7 gap-px bg-border-primary border border-border-primary">
          <button
            v-for="cell in calendarDays"
            :key="cell.key"
            class="relative flex flex-col items-center pt-1.5 pb-1 min-h-[52px] bg-bg-primary transition-colors group"
            :class="[
              cell.month !== 'current' ? 'opacity-25 cursor-default' : 'cursor-pointer hover:bg-bg-sidebar/60',
              cell.month === 'current' && cell.key === todayKey ? 'bg-accent-blue/5' : '',
              cell.month === 'current' && cell.day === selectedDay ? 'bg-bg-sidebar/80 ring-1 ring-inset ring-accent-blue/40' : '',
            ]"
            :disabled="cell.month !== 'current'"
            @click="cell.month === 'current' && (selectedDay = cell.day)"
          >
            <span
              class="text-[10px] font-bold leading-none mb-1.5 w-5 h-5 flex items-center justify-center rounded-full"
              :class="[
                cell.key === todayKey ? 'bg-accent-blue text-white font-black' : 'text-text-secondary',
              ]"
            >
              {{ cell.day }}
            </span>

            <!-- Event dots -->
            <div v-if="eventsByDate[cell.key]?.length" class="flex flex-wrap justify-center gap-px max-w-[32px]">
              <span
                v-for="ev in eventsByDate[cell.key].slice(0, 4)"
                :key="ev.id"
                class="w-1.5 h-1.5 rounded-full flex-none"
                :style="{ backgroundColor: ev.color }"
              />
            </div>
          </button>
        </div>
      </div>

      <!-- Selected day events panel -->
      <div class="w-52 border-l border-border-primary flex flex-col">
        <div class="px-4 py-3 border-b border-border-primary bg-black/10">
          <p class="text-[8px] font-black uppercase tracking-widest text-text-tertiary opacity-50">
            {{ selectedDayKey ? new Date(selectedDayKey + 'T12:00:00').toLocaleDateString('en', { weekday: 'long', day: 'numeric', month: 'short' }) : $t('calendar.selectDay') }}
          </p>
        </div>

        <div class="flex-1 overflow-y-auto p-3 space-y-2">
          <template v-if="selectedEvents.length > 0">
            <div
              v-for="ev in selectedEvents"
              :key="ev.id"
              class="p-2 rounded border text-left"
              :style="{ borderColor: ev.color + '40', backgroundColor: ev.color + '0d' }"
            >
              <div class="flex items-center gap-1.5 mb-0.5">
                <span class="w-1.5 h-1.5 rounded-full flex-none" :style="{ backgroundColor: ev.color }"/>
                <span class="text-[8px] font-black uppercase tracking-widest opacity-60" :style="{ color: ev.color }">
                  {{ ev.type === 'deadline' ? $t('calendar.typeDeadline') : ev.type === 'task' ? $t('calendar.typeTask') : $t('calendar.typeReminder') }}
                </span>
              </div>
              <p class="text-[11px] font-bold text-text-primary leading-tight">{{ ev.title }}</p>
              <p class="text-[9px] text-text-tertiary font-mono mt-0.5 opacity-60">{{ ev.projectName }}</p>
            </div>
          </template>
          <div v-else class="flex flex-col items-center justify-center h-full opacity-25 gap-2 py-8">
            <svg class="w-6 h-6 text-text-tertiary" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1">
              <circle cx="12" cy="12" r="10"/><path d="M12 8v4l3 3"/>
            </svg>
            <p class="text-[9px] text-text-tertiary text-center uppercase tracking-widest font-bold">{{ $t('calendar.noEvents') }}</p>
          </div>
        </div>

        <!-- Upcoming events summary -->
        <div class="border-t border-border-primary p-3">
          <p class="text-[8px] font-black uppercase tracking-widest text-text-tertiary opacity-40 mb-2">{{ $t('calendar.upcomingTitle') }}</p>
          <div class="space-y-1.5">
            <div
              v-for="ev in allEvents.slice(0, 3)"
              :key="ev.id + '-up'"
              class="flex items-center gap-2"
            >
              <span class="w-1.5 h-1.5 rounded-full flex-none" :style="{ backgroundColor: ev.color }"/>
              <span class="text-[9px] text-text-tertiary truncate">{{ ev.title }}</span>
              <span class="text-[8px] text-text-tertiary font-mono opacity-40 flex-none ml-auto">{{ ev.date.slice(5) }}</span>
            </div>
            <p v-if="allEvents.length === 0" class="text-[9px] text-text-tertiary opacity-30 text-center">{{ $t('calendar.noUpcoming') }}</p>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
