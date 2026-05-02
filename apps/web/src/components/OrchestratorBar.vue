<script setup lang="ts">
import { ref } from 'vue';
import type { AISession } from '@atlas/domain';
import QuickPrompts from './QuickPrompts.vue';

const props = defineProps<{
  otherSessions: AISession[];
}>();

const emit = defineEmits<{
  send: [targetId: string, content: string];
  insertPrompt: [text: string];
}>();

const selectedTargetId = ref('');
const messageContent = ref('');

function send() {
  if (!selectedTargetId.value || !messageContent.value) return;
  emit('send', selectedTargetId.value, messageContent.value);
  messageContent.value = '';
}
</script>

<template>
  <div class="flex-none h-10 bg-bg-primary border-t border-border-primary/50 flex items-center px-4 gap-3">
    <div class="flex items-center gap-2 px-2 py-1 bg-white/5 rounded border border-white/10">
      <span class="text-[10px] text-text-tertiary font-bold uppercase tracking-tighter">{{ $t('terminal.orchestrator') }}</span>
      <div class="w-px h-3 bg-white/10" />

      <div class="flex items-center gap-2">
        <select
          v-model="selectedTargetId"
          class="bg-transparent text-[11px] text-text-secondary border-none focus:ring-0 cursor-pointer outline-none max-w-[120px] truncate"
        >
          <option value="" disabled selected>{{ $t('terminal.targetPlaceholder') }}</option>
          <option v-for="s in otherSessions" :key="s.id" :value="s.id">
            {{ s.customName || s.title || s.model }}
          </option>
        </select>

        <input
          v-model="messageContent"
          @keyup.enter="send"
          type="text"
          :placeholder="$t('terminal.sendPlaceholder')"
          class="bg-transparent text-[11px] text-white border-none focus:ring-0 w-64 placeholder:text-text-tertiary"
        />

        <button
          @click="send"
          :disabled="!selectedTargetId || !messageContent"
          class="p-1 hover:text-accent-green disabled:text-text-tertiary transition-colors"
        >
          <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 5l7 7-7 7M5 5l7 7-7 7" />
          </svg>
        </button>
      </div>
    </div>

    <div class="flex-1" />

    <div class="flex items-center gap-2">
      <QuickPrompts @select="$emit('insertPrompt', $event)" />
      <button class="text-[10px] text-text-tertiary hover:text-white uppercase font-bold tracking-tighter transition-colors">
        {{ $t('terminal.syncEnvironment') }}
      </button>
    </div>
  </div>
</template>
