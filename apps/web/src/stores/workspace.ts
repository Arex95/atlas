import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import type { Project, AISession, SessionMessage, AtlasNotification } from '@atlas/domain';
import { AIProvider, SessionMode } from '@atlas/domain';
import { api, ApiError } from '@/api/client';

export interface InboxMessage {
  id?: string;
  fromId: string;
  content: string;
  timestamp?: string;
  isAgent?: boolean;
}

const INBOX_MAX = 100;
const INJECTED_COMMANDS_MAX = 50;

export const useWorkspaceStore = defineStore('workspace', () => {
  const projects = ref<Project[]>([]);
  const tabs = ref<AISession[]>([]);
  const activeTabId = ref<string | null>(null);
  const selectedProjectSlug = ref<string | null>(null);
  const isShowingSessionInfo = ref(false);
  const loading = ref(true);
  const fetchError = ref<string | null>(null);
  const openFiles = ref<{ path: string; name: string }[]>([]);
  const activeFile = ref<string | null>(null);
  const injectedCommands = ref<Record<string, string[]>>({});
  const inboxMessages = ref<Record<string, InboxMessage[]>>({});
  const loadedProjectSlugs = ref<Set<string>>(new Set());
  const notifications = ref<AtlasNotification[]>([]);
  const unreadNotificationCount = ref(0);

  const activeTab = computed(() => tabs.value.find((t) => t.id === activeTabId.value));
  const selectedProject = computed(() =>
    projects.value.find((p) => p.slug === selectedProjectSlug.value),
  );
  const activeFileItem = computed(() =>
    openFiles.value.find((f) => f.path === activeFile.value),
  );

  function logErr(scope: string, e: unknown) {
    if (e instanceof ApiError) {
      console.error(`[Atlas] ${scope}: HTTP ${e.status} — ${e.message}`);
    } else {
      console.error(`[Atlas] ${scope}:`, e);
    }
  }

  async function fetchProjects() {
    fetchError.value = null;
    try {
      projects.value = await api.get<Project[]>('/api/projects');
    } catch (e) {
      logErr('fetchProjects', e);
      fetchError.value = e instanceof ApiError ? e.message : 'Failed to load projects';
    } finally {
      loading.value = false;
    }
  }

  async function createProject(payload: {
    name: string;
    slug: string;
    rootPath: string;
    indexPath?: string;
    description?: string;
    color?: string;
  }) {
    try {
      const created = await api.post<Project>('/api/projects', payload);
      projects.value.unshift(created);
      selectProject(created.slug);
    } catch (e) {
      logErr('createProject', e);
    }
  }

  async function updateProject(
    slug: string,
    payload: {
      name?: string;
      color?: string;
      description?: string;
      rootPath?: string;
      indexPath?: string;
      version?: string;
      author?: string;
    },
  ) {
    try {
      const updated = await api.patch<Project>(`/api/projects/${slug}`, payload);
      const i = projects.value.findIndex((p) => p.slug === slug);
      if (i !== -1) projects.value[i] = updated;
    } catch (e) {
      logErr('updateProject', e);
    }
  }

  async function deleteProject(slug: string) {
    try {
      await api.delete(`/api/projects/${slug}`);
      projects.value = projects.value.filter((p) => p.slug !== slug);
      if (selectedProjectSlug.value === slug) {
        selectedProjectSlug.value = null;
        activeTabId.value = null;
        activeFile.value = null;
      }
      tabs.value = tabs.value.filter((t) => t.projectId !== slug);
    } catch (e) {
      logErr('deleteProject', e);
    }
  }

  async function indexProject(slug: string) {
    try {
      await api.post<unknown>(`/api/projects/${slug}/index`);
      await fetchProjects();
    } catch (e) {
      logErr('indexProject', e);
    }
  }

  async function fetchSavedSessions() {
    try {
      const saved = await api.get<AISession[]>('/api/sessions/saved');
      if (saved.length > 0) {
        const existingIds = new Set(tabs.value.map((t) => t.id));
        const incoming = saved.filter((s) => !existingIds.has(s.id));
        tabs.value = [...tabs.value, ...incoming];
        if (!activeTabId.value) activeTabId.value = saved[0].id;
      }
    } catch (e) {
      logErr('fetchSavedSessions', e);
    }
  }

  async function fetchSessions(projectSlug: string) {
    if (loadedProjectSlugs.value.has(projectSlug)) return;
    try {
      const list = await api.get<AISession[]>(`/api/projects/${projectSlug}/sessions`);
      const newSessions = list.filter((s) => !tabs.value.find((t) => t.id === s.id));
      tabs.value = [...tabs.value, ...newSessions];
      loadedProjectSlugs.value.add(projectSlug);
    } catch (e) {
      logErr('fetchSessions', e);
    }
  }

  async function addLocalTerminal() {
    if (!selectedProjectSlug.value) return;
    try {
      const title = `Terminal - ${new Date().toLocaleTimeString()}`;
      const session = await api.post<AISession>(
        `/api/projects/${selectedProjectSlug.value}/sessions`,
        {
          provider: AIProvider.Bash,
          model: AIProvider.Bash,
          mode: SessionMode.Interactive,
          workingDirectory: selectedProject.value?.rootPath || '',
          title,
        },
      );
      tabs.value.push(session);
      setActiveTab(session.id);
    } catch (e) {
      logErr('addLocalTerminal', e);
    }
  }

  function closeTab(id: string) {
    tabs.value = tabs.value.filter((t) => t.id !== id);
    if (activeTabId.value === id) {
      activeTabId.value =
        tabs.value.length > 0 ? tabs.value[tabs.value.length - 1].id : null;
    }
  }

  async function deleteSession(id: string) {
    try {
      await api.delete(`/api/sessions/${id}`);
      closeTab(id);
      delete inboxMessages.value[id];
      delete injectedCommands.value[id];
    } catch (e) {
      logErr('deleteSession', e);
    }
  }

  async function saveSession(
    sessionId: string,
    payload: {
      customName: string;
      customDescription?: string;
      color?: string;
      icon?: string;
    },
  ) {
    try {
      const updated = await api.post<AISession>(`/api/sessions/${sessionId}/save`, payload);
      const i = tabs.value.findIndex((t) => t.id === sessionId);
      if (i !== -1) tabs.value[i] = updated;
    } catch (e) {
      logErr('saveSession', e);
    }
  }

  async function updateSession(
    id: string,
    payload: { customName?: string; customDescription?: string; color?: string },
  ) {
    try {
      const updated = await api.patch<AISession>(`/api/sessions/${id}`, payload);
      const i = tabs.value.findIndex((t) => t.id === id);
      if (i !== -1) tabs.value[i] = updated;
    } catch (e) {
      logErr('updateSession', e);
    }
  }

  function setActiveTab(id: string) {
    activeTabId.value = id;
    activeFile.value = null;
    isShowingSessionInfo.value = false;
  }

  function showSessionInfo(id: string) {
    activeTabId.value = id;
    isShowingSessionInfo.value = true;
    activeFile.value = null;
  }

  async function selectProject(slug: string) {
    selectedProjectSlug.value = slug;
    activeTabId.value = null;
    activeFile.value = null;
    await fetchSessions(slug);
  }

  async function fetchHistory(sessionId: string) {
    try {
      const history = await api.get<SessionMessage[]>(`/api/sessions/${sessionId}/history`);
      const session = tabs.value.find((t) => t.id === sessionId);
      if (session) session.history = history;
    } catch (e) {
      logErr('fetchHistory', e);
    }
  }

  async function fetchInbox(sessionId: string) {
    try {
      inboxMessages.value[sessionId] = await api.get<InboxMessage[]>(
        `/api/sessions/${sessionId}/messages`,
      );
    } catch (e) {
      logErr('fetchInbox', e);
    }
  }

  function addInboxMessage(sessionId: string, message: InboxMessage) {
    if (!inboxMessages.value[sessionId]) inboxMessages.value[sessionId] = [];
    if (message.id && inboxMessages.value[sessionId].some((m) => m.id === message.id)) return;
    inboxMessages.value[sessionId].unshift(message);
    if (inboxMessages.value[sessionId].length > INBOX_MAX) {
      inboxMessages.value[sessionId].length = INBOX_MAX;
    }
  }

  function openFile(path: string) {
    const name = path.split('/').pop() || path;
    if (!openFiles.value.find((f) => f.path === path)) {
      openFiles.value.push({ path, name });
    }
    activeFile.value = path;
    activeTabId.value = null;
    isShowingSessionInfo.value = false;
  }

  function closeFile(path: string) {
    openFiles.value = openFiles.value.filter((f) => f.path !== path);
    if (activeFile.value === path) {
      activeFile.value =
        openFiles.value.length > 0
          ? openFiles.value[openFiles.value.length - 1].path
          : null;
    }
  }

  function updateSessionPath(sessionId: string, path: string) {
    const session = tabs.value.find((s) => s.id === sessionId);
    if (session) session.workingDirectory = path;
  }

  function sendCommand(sessionId: string, command: string) {
    if (!injectedCommands.value[sessionId]) injectedCommands.value[sessionId] = [];
    if (injectedCommands.value[sessionId].length < INJECTED_COMMANDS_MAX) {
      injectedCommands.value[sessionId].push(command + '\n');
    }
  }

  function consumeCommand(sessionId: string): string | undefined {
    return injectedCommands.value[sessionId]?.shift();
  }

  function addNotification(notif: AtlasNotification) {
    if (notifications.value.some((n) => n.id === notif.id)) return;
    notifications.value.unshift(notif);
    if (notif.status === 'unread') unreadNotificationCount.value++;
  }

  function clearUnreadCount() {
    unreadNotificationCount.value = 0;
  }

  function goHome() {
    selectedProjectSlug.value = null;
    activeTabId.value = null;
    activeFile.value = null;
    isShowingSessionInfo.value = false;
  }

  return {
    projects,
    tabs,
    activeTabId,
    selectedProjectSlug,
    isShowingSessionInfo,
    loading,
    fetchError,
    openFiles,
    activeFile,
    injectedCommands,
    inboxMessages,
    notifications,
    unreadNotificationCount,
    activeTab,
    selectedProject,
    activeFileItem,
    fetchProjects,
    createProject,
    updateProject,
    deleteProject,
    indexProject,
    fetchSavedSessions,
    fetchSessions,
    selectProject,
    addLocalTerminal,
    closeTab,
    deleteSession,
    saveSession,
    updateSession,
    setActiveTab,
    showSessionInfo,
    fetchHistory,
    fetchInbox,
    addInboxMessage,
    openFile,
    closeFile,
    updateSessionPath,
    sendCommand,
    consumeCommand,
    addNotification,
    clearUnreadCount,
    goHome,
  };
});
