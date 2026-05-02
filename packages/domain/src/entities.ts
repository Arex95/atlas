import type {
  ProjectId,
  TaskId,
  SessionId,
  TerminalId,
  EventId,
  TimeEntryId,
  ReminderId,
  WorkflowId,
  IntegrationId,
  NotificationRuleId,
  CalendarEventId,
  ProjectStatus,
  TaskStatus,
  TaskPriority,
  AIProvider,
  SessionStatus,
  SessionMode,
  EventType,
  EventSource,
  TimeEntryType,
  ReminderType,
  IntegrationType,
  CalendarEventType,
  Deadline,
  EventPayload,
  CanvasData,
} from "./value-objects";

export interface GitInfo {
  branch: string;
  hasChanges: boolean;
  insertions: number;
  deletions: number;
  staged: number;
  untracked: number;
  commitHash: string;
  lastCommitMessage: string;
  remoteUrl?: string;
  remoteName?: string;
  userName?: string;
  userEmail?: string;
  ahead: number;
  behind: number;
  stashCount: number;
}

export interface SessionMessage {
  id: string;
  sessionId: string;
  role: "user" | "assistant" | "system";
  content: string;
  createdAt: string;
}

export interface Project {
  id: ProjectId;
  slug: string;
  name: string;
  description: string;
  status: ProjectStatus;
  rootPath: string;
  indexPath: string;
  color?: string;
  deadline: Deadline | null;
  tags: string[];
  createdAt: string;
  updatedAt: string;
  lastSyncedAt: string | null;
  git?: GitInfo;
  author?: string;
  version: string;
}

export interface Task {
  id: TaskId;
  projectId: ProjectId;
  title: string;
  description: string;
  status: TaskStatus;
  priority: TaskPriority;
  deadline: Deadline | null;
  estimatedMinutes: number | null;
  actualMinutes: number;
  assignedSessionId: SessionId | null;
  sourceFile: string | null;
  sourceLine: number | null;
  tags: string[];
  parentId?: string;
  createdAt: string;
  updatedAt: string;
}

export interface AISession {
  id: SessionId;
  projectId: ProjectId;
  provider: AIProvider;
  model: string;
  status: SessionStatus;
  pid: number | null;
  ptyFd: number | null;
  workingDirectory: string;
  prompt: string | null;
  mode: SessionMode;
  linkedTaskId: TaskId | null;
  startedAt: string;
  stoppedAt: string | null;
  lastActivityAt: string;
  title?: string;
  author: string;
  git?: GitInfo;
  history: SessionMessage[];
  isSaved: boolean;
  customName?: string;
  customDescription?: string;
  color?: string;
  icon?: string;
}

export interface Event {
  id: EventId;
  type: EventType;
  projectId: ProjectId | null;
  sessionId: SessionId | null;
  payload: EventPayload;
  emittedAt: string;
  source: EventSource;
}

export interface TimeEntry {
  id: TimeEntryId;
  projectId: ProjectId;
  taskId: TaskId | null;
  sessionId: SessionId | null;
  startedAt: string;
  stoppedAt: string | null;
  durationMinutes: number | null;
  entryType: TimeEntryType;
  note: string | null;
}

export interface Terminal {
  id: TerminalId;
  sessionId: SessionId;
  outputBuffer: Uint8Array | null;
  isPinned: boolean;
  lastFlushAt: string;
}

export interface Reminder {
  id: ReminderId;
  projectId: ProjectId | null;
  type: ReminderType;
  triggerAt: string;
  message: string;
  repeatEveryMinutes: number | null;
  acknowledgedAt: string | null;
  dismissed: boolean;
}

export interface Workflow {
  id: WorkflowId;
  projectId: ProjectId | null;
  name: string;
  description: string | null;
  isActive: boolean;
  canvasData: CanvasData;
  n8nWorkflowId: string | null;
  version: number;
  createdAt: string;
  updatedAt: string;
}

export interface Integration {
  id: IntegrationId;
  type: IntegrationType;
  name: string;
  isActive: boolean;
  credentials: Record<string, unknown>;
  metadata: Record<string, unknown>;
  createdAt: string;
  updatedAt: string;
}

export interface NotificationRule {
  id: NotificationRuleId;
  projectId: ProjectId | null;
  eventType: EventType;
  integrationId: IntegrationId;
  messageTemplate: string;
  isActive: boolean;
  createdAt: string;
}

export interface CalendarEvent {
  id: CalendarEventId;
  projectId: ProjectId | null;
  taskId: TaskId | null;
  sessionId: SessionId | null;
  title: string;
  description: string | null;
  eventType: CalendarEventType;
  startTime: string;
  endTime: string;
  timezone: string;
  isAllDay: boolean;
  color: string | null;
  createdAt: string;
}

export interface StoredPrompt {
  id: string;
  projectId: string | null;
  sessionId: string | null;
  title: string;
  content: string;
  category: string;
  createdAt: string;
  updatedAt: string;
}

export interface ProjectDocument {
  id: string;
  projectId: string;
  title: string;
  content: string;
  type: string;
  tags: string[];
  createdAt: string;
  updatedAt: string;
}

export interface AgentSkill {
  id: string;
  projectId: string | null;
  name: string;
  description: string;
  script: string;
  usageCount: number;
  createdAt: string;
  updatedAt: string;
}

export interface AtlasNotification {
  id: string;
  projectId: string | null;
  sessionId: string | null;
  title: string | null;
  message: string;
  type: string;
  status: string;
  createdAt: string;
}

export interface AtlasReminder {
  id: string;
  projectId: string | null;
  title: string;
  description: string;
  dueAt: string;
  type: string;
  status: string;
  lastNotifiedAt: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface ProjectMetrics {
  totalSessions: number;
  activeSessions: number;
  savedSessions: number;
  totalDocuments: number;
  totalSkills: number;
  totalNotifications: number;
  unreadNotifications: number;
  pendingReminders: number;
  memoryKeys: number;
  totalTasks: number;
  openTasks: number;
  blockedTasks: number;
  doneTasks: number;
  overdueTasks: number;
  healthScore: number;
}

export interface ParsedProjectIndex {
  projectName: string;
  version: string;
  lastUpdated: string;
  status: ProjectStatus;
  description: string;
  deadline: Deadline | null;
  tasks: Array<{
    title: string;
    status: TaskStatus;
    priority: TaskPriority;
    deadline: Deadline | null;
    lineNumber: number;
  }>;
  notes: string[];
  rawContent: string;
}
