<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue';
import { io } from 'socket.io-client';
import type { AISession, AtlasNotification } from '@atlas/domain';
import { useWorkspaceStore } from '@/stores/workspace';
import { useProfileStore } from '@/stores/profile';
import { useToast } from '@/composables/useToast';
import { SERVER_URL } from '@/api/client';
import Sidebar from '@/components/Sidebar.vue';
import TerminalPanel from '@/components/TerminalPanel.vue';
import TopBar from '@/components/TopBar.vue';
import Breadcrumbs from '@/components/Breadcrumbs.vue';
import ProjectDashboard from '@/components/ProjectDashboard.vue';
import SessionDashboard from '@/components/SessionDashboard.vue';
import FileEditor from '@/components/FileEditor.vue';
import HomeDashboard from '@/components/HomeDashboard.vue';
import GlobalSearch from '@/components/GlobalSearch.vue';
import SessionExplorer from './components/Sidebar/SessionExplorer.vue';
import OrchestrationInbox from './components/OrchestrationInbox.vue';
import SessionHistoryPanel from './components/SessionHistoryPanel.vue';
import ConfirmDeleteModal from './components/ConfirmDeleteModal.vue';
import ToastContainer from './components/ToastContainer.vue';

const store = useWorkspaceStore();
const profileStore = useProfileStore();
const { show: showToast } = useToast();
const showExplorer = ref(true);
const showDeleteModal = ref(false);
const sessionToDelete = ref<AISession | null>(null);
const showSearch = ref(false);

function onKeydown(e: KeyboardEvent) {
  if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
    e.preventDefault();
    showSearch.value = !showSearch.value;
  }
}

function requestCloseTab(id: string) {
  const session = store.tabs.find(t => t.id === id);
  if (session) {
    sessionToDelete.value = session;
    showDeleteModal.value = true;
  }
}

async function confirmDelete() {
  if (sessionToDelete.value) {
    await store.deleteSession(sessionToDelete.value.id);
    showDeleteModal.value = false;
    sessionToDelete.value = null;
  }
}

onMounted(async () => {
  await Promise.all([store.fetchProjects(), store.fetchSavedSessions(), profileStore.fetch()]);

  const globalSocket = io(SERVER_URL || window.location.origin, { transports: ['websocket'] });
  globalSocket.on('notification:new', (data: AtlasNotification & { message: string; title?: string; type: string }) => {
    store.addNotification({ ...data, kind: data.type, status: 'unread' } as AtlasNotification);
    showToast(data.message, (data.type as 'info' | 'success' | 'warning' | 'error' | 'reminder') || 'info', data.title ?? undefined);
  });

  window.addEventListener('keydown', onKeydown);
});

onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown);
});
</script>

<template>
  <div class="h-screen flex flex-col bg-bg-primary text-text-primary overflow-hidden font-mono">

    <TopBar version="0.1.0" />

    <div class="flex-1 flex overflow-hidden">
      <Sidebar
        :projects="store.projects"
        :tabs="store.tabs"
        :activeTabId="store.activeTabId"
        :selectedProjectSlug="store.selectedProjectSlug"
        @setActiveTab="store.setActiveTab($event)"
        @selectProject="store.selectProject($event)"
        @showDashboard="store.selectProject($event)"
        @addTerminal="store.addLocalTerminal()"
        @closeTab="requestCloseTab($event)"
        @showSessionInfo="store.showSessionInfo($event)"
        @openFile="store.openFile($event)"
        @terminalCd="store.activeTabId ? store.sendCommand(store.activeTabId, 'cd ' + JSON.stringify($event)) : null"
      />

      <div class="flex-1 bg-bg-primary flex flex-col relative overflow-hidden min-h-0">

        
        <div v-if="store.openFiles.length > 0 || store.tabs.length > 0" class="flex-none bg-black/40 border-b border-white/5 flex items-center overflow-x-auto scrollbar-hide h-9">

          <template v-for="file in store.openFiles" :key="file.path">
            <div 
              @click="store.activeFile = file.path"
              :class="[
                'flex-none h-full flex items-center gap-2 border-r border-border-primary text-[11px] font-medium transition-colors cursor-pointer group',
                store.activeFile === file.path ? 'bg-bg-primary text-white' : 'bg-bg-sidebar text-text-tertiary hover:text-text-secondary'
              ]"
            >
              <div class="flex items-center gap-2 px-4 h-full">
                <span class="opacity-60">📄</span>
                {{ file.name }}
                <button 
                  @click.stop="store.closeFile(file.path)"
                  class="ml-1 p-0.5 hover:bg-white/10 rounded transition-colors opacity-0 group-hover:opacity-100"
                >
                  <svg class="w-2.5 h-2.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="3">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              </div>
            </div>
          </template>
        </div>

        <template v-if="store.activeFileItem">
          <FileEditor :path="store.activeFileItem.path" :name="store.activeFileItem.name" />
        </template>

        <template v-else-if="store.activeTabId">
          <Breadcrumbs 
            :activeTab="store.activeTab ?? null"
            :showExplorer="showExplorer" 
            @toggleExplorer="showExplorer = !showExplorer" 
          />

          <div class="flex-1 overflow-hidden relative min-h-0 flex flex-col">
            <div v-if="store.isShowingSessionInfo && store.activeTab" class="w-full h-full p-12 overflow-y-auto">
              <SessionDashboard 
                :session="store.activeTab"
                @updateSession="store.updateSession(store.activeTab.id, $event)"
                @close="store.isShowingSessionInfo = false"
              />
            </div>

            <template v-else-if="store.activeTab">
              <div class="flex flex-1 overflow-hidden min-h-0 max-h-full">

                <SessionExplorer
                  :key="store.activeTab.id + (store.activeTab.workingDirectory || '')"
                  :rootPath="store.activeTab.workingDirectory || store.selectedProject?.rootPath || ''"
                  @file-selected="store.openFile($event)"
                  @cd-requested="store.sendCommand(store.activeTab.id, 'cd ' + JSON.stringify($event))"
                />

                <div class="flex-1 flex flex-col min-w-0 overflow-hidden min-h-0">
                  <TerminalPanel 
                    v-for="session in store.tabs"
                    :key="session.id"
                    :session="session"
                    :isVisible="store.activeTabId === session.id"
                    @close="store.closeTab(session.id)"
                  />
                </div>

                <SessionHistoryPanel
                  v-if="store.activeTab"
                  :sessionId="store.activeTab.id"
                />
                <OrchestrationInbox
                  v-if="store.activeTab"
                  :sessionId="store.activeTab.id"
                />
              </div>
            </template>
          </div>
        </template>

        <div v-else-if="store.selectedProject" class="flex-1 p-12 overflow-y-auto bg-bg-primary">
          <ProjectDashboard
            :project="store.selectedProject"
            @addTerminal="store.addLocalTerminal()"
            @updateColor="store.updateProject(store.selectedProject.slug, { color: $event })"
            @updateProject="store.updateProject(store.selectedProject.slug, $event)"
            @openSession="store.setActiveTab($event)"
          />
        </div>

        <div v-else class="flex-1 overflow-hidden">
          <HomeDashboard
            :projects="store.projects"
            :sessions="store.tabs"
            @select-project="store.selectProject($event)"
          />
        </div>
      </div>

    </div>

    <ConfirmDeleteModal
      v-if="showDeleteModal && sessionToDelete"
      :id="sessionToDelete.id"
      :name="sessionToDelete.customName || sessionToDelete.title || sessionToDelete.model"
      @close="showDeleteModal = false"
      @confirm="confirmDelete"
    />

    <ToastContainer />

    <GlobalSearch v-if="showSearch" @close="showSearch = false" />
  </div>
</template>
