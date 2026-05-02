<script setup lang="ts">
import type { AISession, GitInfo } from '@atlas/domain';
import { AIProvider } from '@atlas/domain';
import { formatPath } from '@/utils/path';
import { api } from '@/api/client';
import { ref, watch, onMounted, onUnmounted } from 'vue';

const props = defineProps<{
  activeTab: AISession | null;
  showExplorer: boolean;
}>();

defineEmits<{
  toggleExplorer: []
}>();

const gitInfo = ref<GitInfo | null>(null);
let pollInterval: ReturnType<typeof setInterval> | null = null;

async function fetchGit() {
  if (!props.activeTab?.id) {
    gitInfo.value = null;
    return;
  }
  try {
    gitInfo.value = await api.get<GitInfo | null>(`/api/sessions/${props.activeTab.id}/git`);
  } catch {
    gitInfo.value = null;
  }
}

function truncate(str: string, len: number) {
  return str.length > len ? str.slice(0, len) + '…' : str;
}

function shortRemote(url: string) {
  return url.replace(/^https?:\/\//, '').replace(/^git@/, '').replace(/:/, '/').replace(/\.git$/, '');
}

onMounted(() => {
  fetchGit();
  pollInterval = setInterval(fetchGit, 8000);
});

onUnmounted(() => {
  if (pollInterval !== null) clearInterval(pollInterval);
});

watch(() => props.activeTab?.id, () => {
  fetchGit();
});
</script>

<template>
  <div class="h-8 flex-none bg-bg-primary border-b border-border-primary flex items-center px-4 gap-2 select-none">
    <button @click="$emit('toggleExplorer')" :class="['p-1 transition-colors', showExplorer ? 'text-white' : 'text-text-tertiary']">
       <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path d="M4 6h16M4 12h16m-7 6h7"/></svg>
    </button>
    <div class="flex items-center gap-2 overflow-hidden text-[11px]">
      <span class="text-text-tertiary">{{ formatPath(activeTab?.workingDirectory || '') }}</span>
      <span class="text-text-tertiary">/</span>
      <span class="text-text-secondary font-bold">{{ activeTab?.model === AIProvider.Bash ? $t('breadcrumbs.mainBranch') : activeTab?.model }}</span>
    </div>

    <template v-if="gitInfo">
      <div class="flex items-center gap-2 text-[10px] overflow-hidden">
        <span
          :class="[
            'px-1.5 py-0.5 rounded font-medium flex items-center gap-1',
            gitInfo.hasChanges
              ? 'bg-accent-yellow/15 text-accent-yellow'
              : 'bg-accent-green/15 text-accent-green'
          ]"
        >
          <svg class="w-3 h-3 flex-none" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path d="M9 19c-4.418 0-8-1.343-8-3V8m0 0c0-1.657 3.582-3 8-3s8 1.343 8 3m-16 0c0 1.657 3.582 3 8 3s8-1.343 8-3"/></svg>
          {{ gitInfo.branch }}
        </span>

        <span v-if="gitInfo.hasChanges" class="flex items-center gap-1">
          <span class="text-text-tertiary">·</span>
          <span v-if="gitInfo.insertions > 0" class="text-accent-green">+{{ gitInfo.insertions }}</span>
          <span v-if="gitInfo.deletions > 0" class="text-accent-red">-{{ gitInfo.deletions }}</span>
        </span>

        <span v-if="gitInfo.ahead > 0 || gitInfo.behind > 0" class="flex items-center gap-1">
          <span v-if="gitInfo.ahead > 0" class="text-accent-blue">↑{{ gitInfo.ahead }}</span>
          <span v-if="gitInfo.behind > 0" class="text-accent-orange">↓{{ gitInfo.behind }}</span>
        </span>

        <span v-if="gitInfo.commitHash" class="font-mono text-text-tertiary">{{ gitInfo.commitHash }}</span>
        <span v-if="gitInfo.lastCommitMessage" class="text-text-tertiary hidden sm:block">{{ truncate(gitInfo.lastCommitMessage, 35) }}</span>
      </div>

      <div v-if="gitInfo.userName || gitInfo.userEmail || gitInfo.remoteUrl" class="flex items-center gap-1.5 text-text-tertiary text-[9px] ml-1 pl-2 border-l border-border-primary overflow-hidden">
        <svg class="w-2.5 h-2.5 flex-none" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/></svg>
        <span v-if="gitInfo.userName" class="truncate max-w-[80px]">{{ gitInfo.userName }}</span>
        <span v-if="gitInfo.userEmail" class="truncate max-w-[100px]">· {{ truncate(gitInfo.userEmail, 20) }}</span>
        <span v-if="gitInfo.remoteUrl" class="truncate max-w-[120px]">· {{ truncate(shortRemote(gitInfo.remoteUrl), 30) }}</span>
      </div>
    </template>

    <div class="flex-1"></div>
  </div>
</template>
