export type ProjectId = string & { readonly __brand: "ProjectId" };
export type TaskId = string & { readonly __brand: "TaskId" };
export type SessionId = string & { readonly __brand: "SessionId" };
export type TerminalId = string & { readonly __brand: "TerminalId" };
export type EventId = string & { readonly __brand: "EventId" };
export type TimeEntryId = string & { readonly __brand: "TimeEntryId" };
export type ReminderId = string & { readonly __brand: "ReminderId" };
export type WorkflowId = string & { readonly __brand: "WorkflowId" };
export type IntegrationId = string & { readonly __brand: "IntegrationId" };
export type NotificationRuleId = string & { readonly __brand: "NotificationRuleId" };
export type CalendarEventId = string & { readonly __brand: "CalendarEventId" };
export type UserId = string & { readonly __brand: "UserId" };

export const ProjectId = (id: string): ProjectId => id as ProjectId;
export const TaskId = (id: string): TaskId => id as TaskId;
export const SessionId = (id: string): SessionId => id as SessionId;
export const TerminalId = (id: string): TerminalId => id as TerminalId;
export const EventId = (id: string): EventId => id as EventId;
export const TimeEntryId = (id: string): TimeEntryId => id as TimeEntryId;
export const ReminderId = (id: string): ReminderId => id as ReminderId;
export const WorkflowId = (id: string): WorkflowId => id as WorkflowId;
export const IntegrationId = (id: string): IntegrationId => id as IntegrationId;
export const NotificationRuleId = (id: string): NotificationRuleId =>
  id as NotificationRuleId;
export const CalendarEventId = (id: string): CalendarEventId =>
  id as CalendarEventId;
export const UserId = (id: string): UserId => id as UserId;

export const ProjectStatus = {
  Active: "active",
  Paused: "paused",
  Archived: "archived",
} as const;
export type ProjectStatus = (typeof ProjectStatus)[keyof typeof ProjectStatus];

export const TaskStatus = {
  Todo: "todo",
  InProgress: "in-progress",
  Done: "done",
  Blocked: "blocked",
} as const;
export type TaskStatus = (typeof TaskStatus)[keyof typeof TaskStatus];

export const TaskPriority = {
  Low: "low",
  Medium: "medium",
  High: "high",
  Critical: "critical",
} as const;
export type TaskPriority = (typeof TaskPriority)[keyof typeof TaskPriority];

export const AIProvider = {
  Claude: "claude",
  OpenAI: "openai",
  Ollama: "ollama",
  Bash: "bash",
} as const;
export type AIProvider = (typeof AIProvider)[keyof typeof AIProvider];

export const SessionStatus = {
  Starting: "starting",
  Running: "running",
  Idle: "idle",
  Stopped: "stopped",
  Crashed: "crashed",
} as const;
export type SessionStatus = (typeof SessionStatus)[keyof typeof SessionStatus];

export const SessionMode = {
  Interactive: "interactive",
  Autonomous: "autonomous",
} as const;
export type SessionMode = (typeof SessionMode)[keyof typeof SessionMode];

export const EventType = {
  ProjectCreated: "project.created",
  ProjectStatusChanged: "project.status_changed",
  ProjectIndexSynced: "project.index_synced",
  TaskCreated: "task.created",
  TaskStatusChanged: "task.status_changed",
  SessionStarted: "session.started",
  SessionStopped: "session.stopped",
  SessionOutput: "session.output",
  SessionCrashed: "session.crashed",
  TimerStarted: "timer.started",
  TimerStopped: "timer.stopped",
  BreakScheduled: "break.scheduled",
  BreakOverdue: "break.overdue",
  DeadlineWarning: "deadline.warning",
  DeadlineBreached: "deadline.breached",
  WorkflowExecuted: "workflow.executed",
  NotificationSent: "notification.sent",
} as const;
export type EventType = (typeof EventType)[keyof typeof EventType];

export const EventSource = {
  Web: "web",
  CLI: "cli",
  System: "system",
  AISession: "ai-session",
} as const;
export type EventSource = (typeof EventSource)[keyof typeof EventSource];

export const TimeEntryType = {
  Manual: "manual",
  Session: "session",
  Break: "break",
} as const;
export type TimeEntryType = (typeof TimeEntryType)[keyof typeof TimeEntryType];

export const ReminderType = {
  Break: "break",
  Deadline: "deadline",
  Custom: "custom",
} as const;
export type ReminderType = (typeof ReminderType)[keyof typeof ReminderType];

export const IntegrationType = {
  Slack: "slack",
  Discord: "discord",
  Telegram: "telegram",
  Email: "email",
  Webhook: "webhook",
} as const;
export type IntegrationType = (typeof IntegrationType)[keyof typeof IntegrationType];

export const CalendarEventType = {
  Deadline: "deadline",
  Reminder: "reminder",
  SessionScheduled: "session_scheduled",
  Break: "break",
  Custom: "custom",
} as const;
export type CalendarEventType = (typeof CalendarEventType)[keyof typeof CalendarEventType];

export interface Deadline {
  date: string;
  daysRemaining?: number;
  isBreached?: boolean;
  urgency?: "normal" | "warning" | "critical";
}

export interface ProjectIndex {
  projectName: string;
  version: string;
  lastUpdated: string;
  status: ProjectStatus;
  description: string;
  tasks: Array<{
    title: string;
    status: TaskStatus;
    priority: TaskPriority;
    deadline?: string;
  }>;
  notes: string[];
  rawContent: string;
}

export interface EventPayload {
  [key: string]: unknown;
}

export interface TimeRange {
  startTime: string;
  endTime?: string;
  durationMinutes?: number;
}

export interface CanvasNode {
  id: string;
  type: string;
  position: { x: number; y: number };
  data: Record<string, unknown>;
}

export interface CanvasEdge {
  id: string;
  source: string;
  target: string;
}

export interface CanvasData {
  nodes: CanvasNode[];
  edges: CanvasEdge[];
}
