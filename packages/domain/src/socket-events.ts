export const SocketEvent = {
  TERMINAL_OUTPUT: "terminal:output",
  TERMINAL_INPUT: "terminal:input",
  TERMINAL_RESIZE: "terminal:resize",
  TERMINAL_FORCE_WRITE: "terminal:force_write",
  TERMINAL_SECURITY_ALERT: "terminal:security_alert",
  SESSION_MESSAGE: "session:message",
  SESSION_RECEIVE_MESSAGE: "session:receive_message",
  SESSION_UPDATED: "session:updated",
  SESSION_SPAWN: "session:spawn",
  SUBSCRIBE_SESSION: "subscribe:session",
  ORCHESTRATOR_NOTIFICATION: "orchestrator:notification",
} as const;

export type SocketEventName = (typeof SocketEvent)[keyof typeof SocketEvent];
