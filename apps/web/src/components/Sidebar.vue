<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { useWorkspaceStore } from '@/stores/workspace';
import type { Project, AISession } from '@atlas/domain';
import ProjectCard from '@/components/Sidebar/ProjectCard.vue';
import SidebarFooter from '@/components/Sidebar/SidebarFooter.vue';
import NewProjectModal from '@/components/Sidebar/NewProjectModal.vue';
import FileExplorer from '@/components/Sidebar/FileExplorer.vue';

const workspace = useWorkspaceStore();
const showNewProjectModal = ref(false);
const sidebarTab = ref<'sessions' | 'files'>('sessions');

const props = defineProps<{
  projects: Project[];
  tabs: AISession[];
  activeTabId: string | null;
  selectedProjectSlug: string | null;
}>();

const selectedProject = computed(() => {
  return props.projects.find(p => p.slug === props.selectedProjectSlug);
});

const activeTab = computed(() => props.tabs.find(t => t.id === props.activeTabId));
const explorerRootPath = computed(() =>
  activeTab.value?.workingDirectory || selectedProject.value?.rootPath || '',
);

watch(activeTab, () => {
  if (activeTab.value) sidebarTab.value = 'sessions';
});

const emit = defineEmits<{
  setActiveTab: [id: string]
  addTerminal: []
  closeTab: [id: string]
  selectProject: [slug: string]
  showDashboard: [slug: string]
  showSessionInfo: [id: string]
  openFile: [path: string]
  terminalCd: [path: string]
}>();

function sessionsForProject(projectId: string): AISession[] {
  return props.tabs.filter(t => t.projectId === projectId);
}

type CreateProjectPayload = { name: string; slug: string; rootPath: string; indexPath?: string; description?: string; color?: string };

async function handleCreateProject(payload: CreateProjectPayload) {
  await workspace.createProject(payload);
  showNewProjectModal.value = false;
}

</script>

<template>
  <div class="w-64 flex-none bg-bg-sidebar flex flex-col h-full select-none overflow-hidden font-mono border-r border-border-primary">

    <div class="px-4 py-4 flex items-center justify-between border-b border-border-primary mb-1">
      <span class="text-[10px] font-bold text-text-tertiary uppercase tracking-widest">{{ $t('sidebar.title') }}</span>
      <button
        @click="showNewProjectModal = true"
        class="text-text-tertiary hover:text-white transition-colors"
        :title="$t('sidebar.createProject')"
      >
        <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
          <path d="M12 5v14M5 12h14"/>
        </svg>
      </button>
    </div>

    <NewProjectModal 
      v-if="showNewProjectModal" 
      @close="showNewProjectModal = false"
      @create="handleCreateProject"
    />

    <div class="flex-1 flex flex-col overflow-hidden">

      
      <div v-if="selectedProject" class="flex-none px-4 py-3 flex gap-6 border-b border-white/5 bg-black/20">
        <button 
          @click="sidebarTab = 'sessions'"
          :class="['text-[10px] font-black uppercase tracking-[0.2em] transition-all relative', sidebarTab === 'sessions' ? 'text-white' : 'text-text-tertiary hover:text-text-secondary']"
        >
          {{ $t('sidebar.tabs.sessions') }}
          <div v-if="sidebarTab === 'sessions'" class="absolute -bottom-[13px] left-0 right-0 h-0.5 bg-accent-blue" />
        </button>
        <button
          @click="sidebarTab = 'files'"
          :class="['text-[10px] font-black uppercase tracking-[0.2em] transition-all relative', sidebarTab === 'files' ? 'text-white' : 'text-text-tertiary hover:text-text-secondary']"
        >
          {{ $t('sidebar.tabs.files') }}
          <div v-if="sidebarTab === 'files'" class="absolute -bottom-[13px] left-0 right-0 h-0.5 bg-accent-blue" />
        </button>
      </div>

      <div class="flex-1 overflow-y-auto scrollbar-hide py-1">

        <template v-if="sidebarTab === 'sessions'">
          <div v-if="projects.length > 0" class="mb-2">
            <div class="px-4 py-2 text-[9px] font-black text-text-tertiary uppercase tracking-[0.2em] opacity-50">
              {{ $t('sidebar.projects') }}
            </div>
            <ProjectCard
              v-for="project in projects"
              :key="project.id"
              :project="project"
              :isSelected="selectedProjectSlug === project.slug"
              :sessions="sessionsForProject(project.id)"
              :activeTabId="activeTabId"
              @select="emit('selectProject', $event)"
              @openSession="emit('setActiveTab', $event)"
              @addSession="emit('selectProject', $event); emit('addTerminal')"
              @closeSession="emit('closeTab', $event)"
              @showDashboard="emit('showDashboard', $event)"
              @showSessionInfo="emit('showSessionInfo', $event)"
              @deleteProject="workspace.deleteProject($event)"
            />
          </div>
          <div v-else class="px-4 py-8 text-center">
            <p class="text-[11px] text-text-tertiary opacity-50">{{ $t('sidebar.noProjects') }}</p>
          </div>
        </template>

        <template v-else-if="sidebarTab === 'files' && selectedProject">
          <div class="px-4 py-2 text-[9px] font-black text-text-tertiary uppercase tracking-[0.2em] opacity-50">
            {{ $t('sidebar.explorer') }}
          </div>
          <FileExplorer
            :project="selectedProject"
            :active-path="explorerRootPath"
            @file-selected="emit('openFile', $event)"
            @cd-requested="emit('terminalCd', $event)"
          />
        </template>
      </div>
    </div>

    <SidebarFooter />
  </div>
</template>

<style scoped>
.scrollbar-hide::-webkit-scrollbar {
  display: none;
}
.scrollbar-hide {
  -ms-overflow-style: none;
  scrollbar-width: none;
}
</style>
