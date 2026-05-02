<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import type { ProjectMetrics } from '@atlas/domain';
import { api } from '@/api/client';

const props = defineProps<{ projectSlug: string }>();

const metrics = ref<ProjectMetrics | null>(null);
const loading = ref(false);

async function load() {
  loading.value = true;
  try {
    metrics.value = await api.get<ProjectMetrics>(`/api/projects/${props.projectSlug}/metrics`);
  } finally {
    loading.value = false;
  }
}

onMounted(load);

interface Stat {
  label: string;
  value: number;
  color: string;
}

function stats(): Stat[] {
  if (!metrics.value) return [];
  return [
    { label: 'Total Sessions', value: metrics.value.totalSessions, color: 'text-accent-blue' },
    { label: 'Active Sessions', value: metrics.value.activeSessions, color: 'text-accent-green' },
    { label: 'Saved Sessions', value: metrics.value.savedSessions, color: 'text-accent-purple' },
    { label: 'Documents', value: metrics.value.totalDocuments, color: 'text-accent-yellow' },
    { label: 'Skills', value: metrics.value.totalSkills, color: 'text-accent-cyan' },
    { label: 'Notifications', value: metrics.value.totalNotifications, color: 'text-text-tertiary' },
    { label: 'Unread', value: metrics.value.unreadNotifications, color: 'text-accent-red' },
    { label: 'Pending Reminders', value: metrics.value.pendingReminders, color: 'text-accent-orange' },
    { label: 'Memory Keys', value: metrics.value.memoryKeys, color: 'text-text-secondary' },
    { label: 'Total Tasks', value: metrics.value.totalTasks, color: 'text-text-primary' },
    { label: 'Open Tasks', value: metrics.value.openTasks, color: 'text-accent-blue' },
    { label: 'Blocked Tasks', value: metrics.value.blockedTasks, color: 'text-accent-red' },
    { label: 'Done Tasks', value: metrics.value.doneTasks, color: 'text-accent-green' },
    { label: 'Overdue Tasks', value: metrics.value.overdueTasks, color: 'text-accent-orange' },
  ];
}

const healthColor = computed(() => {
  const s = metrics.value?.healthScore ?? 100;
  if (s >= 80) return 'text-accent-green';
  if (s >= 50) return 'text-accent-yellow';
  return 'text-accent-red';
});

const healthBarColor = computed(() => {
  const s = metrics.value?.healthScore ?? 100;
  if (s >= 80) return 'var(--color-accent-green)';
  if (s >= 50) return 'var(--color-accent-yellow)';
  return 'var(--color-accent-red)';
});
</script>

<template>
  <div class="h-full flex flex-col gap-4">
    <div class="flex items-center justify-between">
      <span class="text-[9px] font-black uppercase tracking-widest text-text-tertiary">{{ $t('metrics.title') }}</span>
      <button class="text-[9px] font-black uppercase tracking-wider text-text-tertiary hover:text-text-primary transition-colors" @click="load">
        {{ $t('metrics.refresh') }}
      </button>
    </div>

    <div v-if="loading" class="flex items-center justify-center p-12 opacity-40">
      <div class="w-4 h-4 border border-accent-blue border-t-transparent rounded-full animate-spin" />
    </div>

    <div v-else-if="metrics" class="flex flex-col gap-4">
      <!-- Health score banner -->
      <div class="bg-bg-elevated/30 border border-border-primary rounded-lg p-5 flex items-center gap-6">
        <div class="flex flex-col items-center gap-1">
          <span class="text-[9px] font-black uppercase tracking-widest text-text-tertiary opacity-60">Health Score</span>
          <span class="text-5xl font-black" :class="healthColor">{{ metrics.healthScore }}</span>
          <span class="text-[9px] text-text-tertiary font-mono">/100</span>
        </div>
        <div class="flex-1 flex flex-col gap-2">
          <div class="h-3 bg-bg-sidebar/60 rounded-full overflow-hidden">
            <div
              class="h-full rounded-full transition-all duration-500"
              :style="{ width: metrics.healthScore + '%', backgroundColor: healthBarColor }"
            />
          </div>
          <div class="flex gap-4 flex-wrap">
            <span v-if="metrics.blockedTasks > 0" class="text-[9px] font-mono text-accent-red">{{ metrics.blockedTasks }} blocked</span>
            <span v-if="metrics.overdueTasks > 0" class="text-[9px] font-mono text-accent-orange">{{ metrics.overdueTasks }} overdue</span>
            <span v-if="metrics.activeSessions > 0" class="text-[9px] font-mono text-accent-green">{{ metrics.activeSessions }} active sessions</span>
            <span v-if="metrics.blockedTasks === 0 && metrics.overdueTasks === 0" class="text-[9px] font-mono text-accent-green">No blockers</span>
          </div>
        </div>
      </div>

      <!-- Stat grid -->
      <div class="grid grid-cols-3 gap-3">
        <div
          v-for="stat in stats()"
          :key="stat.label"
          class="bg-bg-elevated/30 border border-border-primary rounded-lg p-4 flex flex-col gap-1"
        >
          <span class="text-[9px] font-black uppercase tracking-wider text-text-tertiary opacity-60">{{ stat.label }}</span>
          <span class="text-2xl font-black" :class="stat.color">{{ stat.value }}</span>
        </div>
      </div>
    </div>
  </div>
</template>
