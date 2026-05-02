<script setup lang="ts">
import { computed } from 'vue';
import type { Project, AISession } from '@atlas/domain';
import WorkspaceCalendar from './WorkspaceCalendar.vue';

const props = defineProps<{
  projects: Project[];
  sessions: AISession[];
}>();

const emit = defineEmits<{
  selectProject: [slug: string]
}>();

// ── Stats ────────────────────────────────────────────────────────────────────
const activeSessions = computed(() => props.sessions.filter(s => s.status === 'running'));
const savedSessions = computed(() => props.sessions.filter(s => s.isSaved));
const activeProjects = computed(() => props.projects.filter(p => p.status === 'active'));

// ── Chart: Sessions per project (horizontal bars) ────────────────────────────
const sessionsByProject = computed(() =>
  props.projects.map(p => ({
    ...p,
    count: props.sessions.filter(s => s.projectId === p.id).length,
  }))
);
const maxSessionCount = computed(() => Math.max(...sessionsByProject.value.map(p => p.count), 1));

// ── Chart: Project status donut ───────────────────────────────────────────────
const statusGroups = computed(() => {
  const active = props.projects.filter(p => p.status === 'active').length;
  const paused = props.projects.filter(p => p.status === 'paused').length;
  const archived = props.projects.filter(p => p.status === 'archived').length;
  return { active, paused, archived };
});

// SVG donut — r=15.915 → circumference ≈ 100
const DONUT_R = 15.915;
const DONUT_CIRC = 2 * Math.PI * DONUT_R; // ≈ 100

function donutSegments(groups: { active: number; paused: number; archived: number }) {
  const total = groups.active + groups.paused + groups.archived || 1;
  const segments = [
    { label: 'Active', value: groups.active, color: '#10b981' },
    { label: 'Paused', value: groups.paused, color: '#4b5563' },
    { label: 'Archived', value: groups.archived, color: '#f59e0b' },
  ];
  let offset = -25; // start at top
  return segments.map(seg => {
    const pct = (seg.value / total) * DONUT_CIRC;
    const res = { ...seg, dasharray: `${pct} ${DONUT_CIRC - pct}`, dashoffset: -offset };
    offset += pct;
    return res;
  });
}

const donut = computed(() => donutSegments(statusGroups.value));

// ── Chart: Sessions by status (mini bar chart) ────────────────────────────────
const sessionStatuses = computed(() => {
  const counts: Record<string, number> = {};
  for (const s of props.sessions) {
    counts[s.status] = (counts[s.status] || 0) + 1;
  }
  const order = ['running', 'idle', 'stopped', 'error'];
  const colors: Record<string, string> = {
    running: '#10b981', idle: '#3b82f6', stopped: '#6b7280', error: '#ef4444',
  };
  const max = Math.max(...Object.values(counts), 1);
  return order
    .filter(k => props.sessions.length > 0 || k === 'stopped')
    .map(k => ({ label: k, count: counts[k] || 0, color: colors[k], pct: ((counts[k] || 0) / max) * 100 }));
});

// ── Project status badge ──────────────────────────────────────────────────────
const statusColor: Record<string, string> = {
  active: 'bg-accent-green',
  inactive: 'bg-text-tertiary/30',
  archived: 'bg-yellow-500/50',
};
</script>

<template>
  <div class="w-full h-full overflow-y-auto scrollbar-hide">
    <div class="max-w-5xl mx-auto p-10 space-y-8">

      <!-- Title -->
      <div>
        <p class="text-[9px] font-black uppercase tracking-[0.4em] text-text-tertiary opacity-40 mb-1">{{ $t('homeDashboard.subtitle') }}</p>
        <h1 class="text-4xl font-black tracking-tighter text-white uppercase">{{ $t('homeDashboard.title') }}</h1>
      </div>

      <!-- Stat cards -->
      <div class="grid grid-cols-4 gap-3">
        <div class="bg-bg-sidebar/40 border border-border-primary rounded-lg p-5">
          <p class="text-[9px] font-black uppercase tracking-widest text-text-tertiary opacity-50 mb-2">{{ $t('homeDashboard.statProjects') }}</p>
          <p class="text-3xl font-black text-white">{{ projects.length }}</p>
          <p class="text-[9px] text-text-tertiary opacity-40 mt-1 font-mono">{{ $t('homeDashboard.statProjectsActive', { count: activeProjects.length }) }}</p>
        </div>
        <div class="bg-bg-sidebar/40 border border-border-primary rounded-lg p-5">
          <p class="text-[9px] font-black uppercase tracking-widest text-text-tertiary opacity-50 mb-2">{{ $t('homeDashboard.statSessions') }}</p>
          <p class="text-3xl font-black text-white">{{ sessions.length }}</p>
          <p class="text-[9px] text-text-tertiary opacity-40 mt-1 font-mono">{{ $t('homeDashboard.statSessionsSaved', { count: savedSessions.length }) }}</p>
        </div>
        <div class="bg-bg-sidebar/40 border border-border-primary rounded-lg p-5">
          <p class="text-[9px] font-black uppercase tracking-widest text-text-tertiary opacity-50 mb-2">{{ $t('homeDashboard.statRunning') }}</p>
          <p class="text-3xl font-black text-accent-green">{{ activeSessions.length }}</p>
          <p class="text-[9px] text-text-tertiary opacity-40 mt-1 font-mono">{{ $t('homeDashboard.statRunningActive') }}</p>
        </div>
        <div class="bg-bg-sidebar/40 border border-border-primary rounded-lg p-5">
          <p class="text-[9px] font-black uppercase tracking-widest text-text-tertiary opacity-50 mb-2">{{ $t('homeDashboard.statDeadlines') }}</p>
          <p class="text-3xl font-black text-yellow-400">{{ projects.filter(p => p.deadline?.date).length }}</p>
          <p class="text-[9px] text-text-tertiary opacity-40 mt-1 font-mono">{{ $t('homeDashboard.statDeadlinesWith') }}</p>
        </div>
      </div>

      <!-- Charts row -->
      <div class="grid grid-cols-3 gap-4">

        <!-- Sessions per project (horizontal bar chart) -->
        <div class="bg-bg-sidebar/30 border border-border-primary rounded-lg p-5 col-span-2">
          <p class="text-[9px] font-black uppercase tracking-[0.3em] text-text-tertiary opacity-50 mb-5">{{ $t('homeDashboard.chartSessionsPerProject') }}</p>
          <div v-if="projects.length > 0" class="space-y-3">
            <div v-for="p in sessionsByProject" :key="p.id" class="flex items-center gap-3">
              <span class="text-[10px] font-bold text-text-secondary w-24 truncate flex-none text-right font-mono uppercase tracking-tight" :title="p.name">
                {{ p.name.slice(0, 10) }}
              </span>
              <div class="flex-1 h-5 bg-black/30 rounded-sm overflow-hidden relative">
                <div
                  class="h-full rounded-sm transition-all duration-700"
                  :style="{ width: `${(p.count / maxSessionCount) * 100}%`, backgroundColor: p.color || '#3b82f6' }"
                />
                <span class="absolute right-2 top-0 h-full flex items-center text-[9px] font-black text-white/40 font-mono">
                  {{ p.count }}
                </span>
              </div>
            </div>
          </div>
          <div v-else class="flex items-center justify-center h-20 opacity-20">
            <p class="text-[10px] text-text-tertiary uppercase tracking-widest">{{ $t('homeDashboard.chartNoProjects') }}</p>
          </div>
        </div>

        <!-- Project status donut -->
        <div class="bg-bg-sidebar/30 border border-border-primary rounded-lg p-5">
          <p class="text-[9px] font-black uppercase tracking-[0.3em] text-text-tertiary opacity-50 mb-4">{{ $t('homeDashboard.chartProjectHealth') }}</p>
          <div class="flex flex-col items-center gap-4">
            <div class="relative w-24 h-24">
              <svg viewBox="0 0 42 42" class="w-full h-full -rotate-90">
                <circle cx="21" cy="21" :r="DONUT_R" fill="none" stroke="rgba(255,255,255,0.05)" stroke-width="4"/>
                <circle
                  v-for="seg in donut"
                  :key="seg.label"
                  cx="21" cy="21"
                  :r="DONUT_R"
                  fill="none"
                  :stroke="seg.color"
                  stroke-width="4"
                  stroke-linecap="butt"
                  :stroke-dasharray="seg.dasharray"
                  :stroke-dashoffset="seg.dashoffset"
                />
              </svg>
              <div class="absolute inset-0 flex flex-col items-center justify-center">
                <span class="text-xl font-black text-white">{{ projects.length }}</span>
                <span class="text-[8px] text-text-tertiary opacity-50 uppercase tracking-widest font-bold">{{ $t('homeDashboard.chartTotal') }}</span>
              </div>
            </div>

            <div class="space-y-1.5 w-full">
              <div v-for="seg in donut" :key="seg.label" class="flex items-center justify-between">
                <div class="flex items-center gap-2">
                  <span class="w-2 h-2 rounded-full flex-none" :style="{ backgroundColor: seg.color }"/>
                  <span class="text-[9px] text-text-tertiary uppercase tracking-wider font-bold">{{ $t(`homeDashboard.donut${seg.label}`) }}</span>
                </div>
                <span class="text-[10px] font-black text-text-secondary font-mono">{{ seg.value }}</span>
              </div>
            </div>
          </div>
        </div>

      </div>

      <!-- Session status breakdown (if sessions exist) -->
      <div v-if="sessions.length > 0" class="bg-bg-sidebar/30 border border-border-primary rounded-lg p-5">
        <p class="text-[9px] font-black uppercase tracking-[0.3em] text-text-tertiary opacity-50 mb-4">{{ $t('homeDashboard.chartSessionBreakdown') }}</p>
        <div class="flex items-end gap-3 h-16">
          <div v-for="s in sessionStatuses" :key="s.label" class="flex-1 flex flex-col items-center gap-1.5">
            <span class="text-[10px] font-black text-text-secondary font-mono">{{ s.count }}</span>
            <div class="w-full rounded-t-sm transition-all duration-700" :style="{ height: `${Math.max(s.pct * 0.48, s.count > 0 ? 4 : 2)}px`, backgroundColor: s.color, opacity: s.count === 0 ? '0.2' : '0.9' }" />
            <span class="text-[8px] font-black uppercase tracking-widest font-mono" :style="{ color: s.color, opacity: s.count === 0 ? '0.3' : '1' }">{{ s.label }}</span>
          </div>
        </div>
      </div>

      <!-- Calendar -->
      <div>
        <h2 class="text-[10px] font-black uppercase tracking-[0.3em] text-text-tertiary opacity-60 mb-3">{{ $t('homeDashboard.calendarTitle') }}</h2>
        <WorkspaceCalendar :projects="projects" />
      </div>

      <!-- Project list -->
      <div>
        <h2 class="text-[10px] font-black uppercase tracking-[0.3em] text-text-tertiary opacity-60 mb-3">{{ $t('homeDashboard.projectsTitle') }}</h2>
        <div class="space-y-2">
          <button
            v-for="project in projects"
            :key="project.id"
            class="w-full flex items-center gap-4 p-4 bg-bg-sidebar/30 border border-border-primary rounded-lg hover:bg-bg-sidebar/60 hover:border-white/10 transition-all group text-left"
            @click="emit('selectProject', project.slug)"
          >
            <div
              class="w-8 h-8 rounded flex-none flex items-center justify-center border"
              :style="{ backgroundColor: (project.color || '#3b82f6') + '1a', borderColor: (project.color || '#3b82f6') + '33' }"
            >
              <svg class="w-4 h-4" :style="{ color: project.color || '#3b82f6' }" fill="currentColor" viewBox="0 0 20 20">
                <path d="M2 6a2 2 0 012-2h4l2 2h4a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z"/>
              </svg>
            </div>
            <div class="flex-1 min-w-0">
              <p class="text-[13px] font-bold text-text-primary group-hover:text-white transition-colors truncate uppercase tracking-tight">{{ project.name }}</p>
              <p class="text-[10px] text-text-tertiary font-mono opacity-60 truncate mt-0.5">{{ project.rootPath }}</p>
            </div>
            <div class="flex items-center gap-4 flex-none">
              <div v-if="project.deadline?.date" class="flex items-center gap-1.5">
                <svg class="w-3 h-3 text-yellow-400 opacity-70" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                  <circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/>
                </svg>
                <span class="text-[9px] font-mono text-yellow-400/70">{{ project.deadline.date.slice(0, 10) }}</span>
              </div>
              <div class="flex items-center gap-1.5">
                <div class="w-1.5 h-1.5 rounded-full" :class="statusColor[project.status] || 'bg-text-tertiary/30'" />
                <span class="text-[8px] font-black uppercase tracking-widest text-text-tertiary">{{ project.status }}</span>
              </div>
              <svg class="w-4 h-4 text-text-tertiary opacity-0 group-hover:opacity-60 transition-opacity" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M9 5l7 7-7 7"/></svg>
            </div>
          </button>
        </div>

        <div v-if="projects.length === 0" class="flex flex-col items-center justify-center py-16 opacity-30">
          <svg class="w-12 h-12 text-text-tertiary mb-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1">
            <path d="M3 7a2 2 0 012-2h4l2 2h8a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2V7z"/>
          </svg>
          <p class="text-[11px] text-text-tertiary uppercase tracking-widest font-bold">{{ $t('homeDashboard.noProjects') }}</p>
          <p class="text-[10px] text-text-tertiary opacity-60 mt-1">{{ $t('homeDashboard.noProjectsHint') }}</p>
        </div>
      </div>

    </div>
  </div>
</template>

<style scoped>
.scrollbar-hide::-webkit-scrollbar { display: none; }
.scrollbar-hide { -ms-overflow-style: none; scrollbar-width: none; }
</style>
