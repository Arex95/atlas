<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, watch, computed } from 'vue';
import { useI18n } from 'vue-i18n';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { io, Socket } from 'socket.io-client';
import type { AISession } from '@atlas/domain';
import { SocketEvent } from '@atlas/domain';
import { useWorkspaceStore } from '@/stores/workspace';
import { SERVER_URL } from '@/api/client';
import SecurityAlertDialog from './SecurityAlertDialog.vue';
import IncomingMessageToast from './IncomingMessageToast.vue';
import OrchestratorBar from './OrchestratorBar.vue';
import '@xterm/xterm/css/xterm.css';

const { t } = useI18n();

interface TerminalOutputEvent { sessionId: string; output: string }
interface SessionMessageEvent {
  id?: string;
  fromId: string;
  content: string;
  timestamp?: string;
  isAgent?: boolean;
}
interface SecurityAlertEvent { sessionId: string; command: string }
interface SessionUpdatedEvent { sessionId: string; workingDirectory: string }

const props = defineProps<{
  session: AISession;
  isVisible: boolean;
}>();

const emit = defineEmits<{ close: [] }>();

const store = useWorkspaceStore();
const isAgentActive = ref(false);
const showSecurityAlert = ref(false);
const pendingCommand = ref('');
const lastMessage = ref<{ from: string; content: string; isAgent?: boolean } | null>(null);
const showNotification = ref(false);
const terminalRef = ref<HTMLDivElement | null>(null);
const isConnected = ref(false);

const otherSessions = computed(() =>
  store.tabs.filter(t => t.id !== props.session.id && t.projectId === props.session.projectId)
);

function readToken(name: string, fallback: string): string {
  if (typeof window === 'undefined') return fallback;
  const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return v || fallback;
}

let socket: Socket | null = null;
let term: Terminal | null = null;
let fitAddon: FitAddon | null = null;
let resizeObserver: ResizeObserver | null = null;
let unwatchVisible: (() => void) | null = null;
let unwatchCommands: (() => void) | null = null;

function sendInterSessionMessage(targetId: string, content: string) {
  if (!socket?.connected) return;
  socket.emit(SocketEvent.SESSION_MESSAGE, {
    fromId: props.session.customName || props.session.title || props.session.model,
    toId: targetId,
    content,
  });
  term?.write(`\r\n\x1b[1;32m${t('terminal.messages.sent', { target: targetId })}\x1b[0m ${content}\r\n`);
}

function insertPromptText(text: string) {
  if (socket?.connected) {
    socket.emit(SocketEvent.TERMINAL_INPUT, { sessionId: props.session.id, data: text });
  }
}

function approveCommand() {
  if (socket?.connected && pendingCommand.value) {
    socket.emit(SocketEvent.TERMINAL_FORCE_WRITE, { sessionId: props.session.id, data: pendingCommand.value + '\n' });
    showSecurityAlert.value = false;
    pendingCommand.value = '';
  }
}

function cancelCommand() {
  showSecurityAlert.value = false;
  pendingCommand.value = '';
  term?.write(`\r\n\x1b[1;31m${t('terminal.messages.securityCancelled')}\x1b[0m\r\n`);
}

function focusTerminal() {
  term?.focus();
}

function resetTerminal() {
  if (!socket?.connected) return;
  // Ctrl+C to interrupt any stuck process, then reset terminal state
  socket.emit(SocketEvent.TERMINAL_INPUT, { sessionId: props.session.id, data: '\x03' });
  setTimeout(() => {
    socket?.emit(SocketEvent.TERMINAL_INPUT, { sessionId: props.session.id, data: 'reset\r' });
    term?.focus();
  }, 100);
}

onMounted(() => {
  if (!terminalRef.value) return;

  term = new Terminal({
    cursorBlink: true,
    theme: {
      background: readToken('--color-bg-primary', '#0c0c0c'),
      foreground: readToken('--color-text-primary', '#e0e0e0'),
      cursor: readToken('--color-text-tertiary', '#555555'),
      selectionBackground: 'rgba(255, 255, 255, 0.1)',
    },
    fontSize: 13,
    fontFamily: '"JetBrains Mono", "Monaco", "Menlo", "Ubuntu Mono", monospace',
    lineHeight: 1.6,
    letterSpacing: 0.5,
    cols: 80,
    rows: 24,
    scrollback: 10000,
  });

  fitAddon = new FitAddon();
  term.loadAddon(fitAddon);
  term.open(terminalRef.value);

  term.attachCustomKeyEventHandler((event) => {
    if (event.key === 'F12' || (event.ctrlKey && event.shiftKey && event.key === 'I')) return false;
    return true;
  });

  setTimeout(() => {
    fitAddon?.fit();
    term?.focus();
    if (term && socket?.connected) {
      socket.emit(SocketEvent.TERMINAL_RESIZE, { sessionId: props.session.id, rows: term.rows, cols: term.cols });
    }
  }, 100);

  socket = io(SERVER_URL || window.location.origin, {
    transports: ['websocket'],
    reconnection: true,
    reconnectionAttempts: Infinity,
    reconnectionDelay: 1000,
    reconnectionDelayMax: 10000,
  });

  socket.on('connect', () => {
    isConnected.value = true;
    socket?.emit(SocketEvent.SUBSCRIBE_SESSION, props.session.id);
    socket?.emit(SocketEvent.SESSION_SPAWN, { sessionId: props.session.id });
  });

  socket.on('connect_error', () => { isConnected.value = false; });
  socket.on('disconnect', () => { isConnected.value = false; });

  socket.on(SocketEvent.TERMINAL_OUTPUT, (event: TerminalOutputEvent) => {
    if (event.sessionId !== props.session.id || !term) return;
    const isAtBottom = term.buffer.active.viewportY >= term.buffer.active.baseY - 1;
    term.write(event.output);
    if (isAtBottom) term.scrollToBottom();
  });

  socket.on(SocketEvent.SESSION_RECEIVE_MESSAGE, (event: SessionMessageEvent) => {
    const isAgent = event.isAgent === true;
    if (isAgent) {
      isAgentActive.value = true;
      setTimeout(() => { isAgentActive.value = false; }, 3000);
    }

    store.addInboxMessage(props.session.id, {
      id: event.id,
      fromId: event.fromId,
      content: event.content,
      timestamp: event.timestamp || new Date().toISOString(),
    });

    if (term) {
      term.writeln('\r\n');
      term.writeln(`\x1b[41;37;1m ${isAgent ? t('terminal.messages.agent') : t('terminal.messages.orchestrator')} \x1b[0m`);
      term.writeln(`\x1b[1;34m${t('terminal.from')} ${event.fromId}:\x1b[0m ${event.content}`);
      term.writeln('\r\n');
      term.refresh(0, term.rows - 1);
      term.scrollToBottom();
    }

    lastMessage.value = { from: event.fromId, content: event.content, isAgent };
    showNotification.value = true;
    setTimeout(() => { showNotification.value = false; }, 8000);
  });

  socket.on(SocketEvent.TERMINAL_SECURITY_ALERT, (data: SecurityAlertEvent) => {
    if (data.sessionId !== props.session.id) return;
    pendingCommand.value = data.command;
    showSecurityAlert.value = true;
  });

  socket.on(SocketEvent.SESSION_UPDATED, (data: SessionUpdatedEvent) => {
    if (data.sessionId === props.session.id) {
      store.updateSessionPath(data.sessionId, data.workingDirectory);
    }
  });

  term.onData((data: string) => {
    if (socket?.connected) {
      socket.emit(SocketEvent.TERMINAL_INPUT, { sessionId: props.session.id, data });
    }
  });

  resizeObserver = new ResizeObserver(() => {
    if (props.isVisible) {
      fitAddon?.fit();
      if (term && socket?.connected) {
        socket.emit(SocketEvent.TERMINAL_RESIZE, { sessionId: props.session.id, rows: term.rows, cols: term.cols });
      }
    }
  });

  if (terminalRef.value) resizeObserver.observe(terminalRef.value);

  unwatchVisible = watch(() => props.isVisible, (visible) => {
    if (visible) setTimeout(() => { fitAddon?.fit(); term?.focus(); }, 100);
  });

  unwatchCommands = watch(() => store.injectedCommands[props.session.id]?.length, () => {
    const cmd = store.consumeCommand(props.session.id);
    if (cmd && socket?.connected) {
      socket.emit(SocketEvent.TERMINAL_INPUT, { sessionId: props.session.id, data: cmd });
    }
  });
});

onBeforeUnmount(() => {
  unwatchVisible?.();
  unwatchCommands?.();
  resizeObserver?.disconnect();
  socket?.disconnect();
  socket = null;
  term?.dispose();
  term = null;
});
</script>

<template>
  <div v-show="isVisible" class="flex flex-col flex-1 overflow-hidden min-h-0 bg-bg-primary font-mono">

    <div class="flex-none h-8 flex items-center justify-between bg-bg-sidebar border-b border-border-primary px-4">
      <div class="flex items-center gap-4">
        <div class="flex items-center gap-2">
          <div :class="['w-1.5 h-1.5 rounded-full', isConnected ? 'bg-accent-green' : 'bg-accent-red']" />
          <span class="text-[10px] text-text-tertiary uppercase font-bold tracking-widest">
            {{ isConnected ? $t('terminal.connected') : $t('terminal.disconnected') }}
          </span>
        </div>
        <div class="flex items-center gap-2 text-[11px] text-text-secondary">
          <span class="font-bold">{{ session.model }}</span>
          <span class="text-text-tertiary">/</span>
          <span class="text-text-tertiary">{{ session.mode }}</span>
        </div>
        <div v-if="isAgentActive" class="flex items-center gap-2 px-2 py-0.5 bg-accent-blue/10 rounded border border-accent-blue/20 animate-pulse">
          <div class="w-1.5 h-1.5 rounded-full bg-accent-blue" />
          <span class="text-[9px] text-accent-blue font-bold uppercase tracking-widest">{{ $t('terminal.agentActive') }}</span>
        </div>
      </div>
      <div class="flex items-center gap-2">
        <button
          @click="resetTerminal"
          :disabled="!isConnected"
          class="p-1 text-text-tertiary hover:text-accent-yellow disabled:opacity-30 transition-colors"
          title="Reset terminal (Ctrl+C + reset)"
        >
          <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
          </svg>
        </button>
      </div>
    </div>

    <div class="flex-1 overflow-hidden min-h-0 bg-bg-primary relative" @click="focusTerminal">
      <div ref="terminalRef" class="absolute inset-4" />
    </div>

    <OrchestratorBar
      :other-sessions="otherSessions"
      @send="sendInterSessionMessage"
      @insert-prompt="insertPromptText"
    />

    <IncomingMessageToast
      v-if="showNotification && lastMessage"
      :from="lastMessage.from"
      :content="lastMessage.content"
      :is-agent="lastMessage.isAgent"
    />

    <SecurityAlertDialog
      v-if="showSecurityAlert"
      :command="pendingCommand"
      @approve="approveCommand"
      @cancel="cancelCommand"
    />
  </div>
</template>

<style scoped>
:deep(.xterm-viewport) {
  background-color: var(--color-bg-primary) !important;
}
:deep(.xterm-screen) {
  padding: 0 !important;
}
:deep(.xterm-viewport::-webkit-scrollbar) {
  width: 10px;
}
:deep(.xterm-viewport::-webkit-scrollbar-track) {
  background: rgba(0, 0, 0, 0.2);
}
:deep(.xterm-viewport::-webkit-scrollbar-thumb) {
  background: rgba(255, 255, 255, 0.1);
  border-radius: 10px;
  border: 2px solid transparent;
  background-clip: padding-box;
}
:deep(.xterm-viewport::-webkit-scrollbar-thumb:hover) {
  background: rgba(255, 255, 255, 0.2);
  border: 2px solid transparent;
  background-clip: padding-box;
}
</style>
