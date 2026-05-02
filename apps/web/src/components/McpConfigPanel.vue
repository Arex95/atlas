<script setup lang="ts">
import { ref, computed } from 'vue';
import { SERVER_URL } from '@/api/client';

const emit = defineEmits<{ close: [] }>();

const copied = ref<string | null>(null);
const token = ref('');
const collapsed = ref(false);

const dotMcpJson = computed(() => {
  const obj: Record<string, unknown> = {
    mcpServers: {
      atlas: {
        type: 'http',
        url: `${SERVER_URL}/api/mcp`,
      },
    },
  };
  if (token.value.trim()) {
    (obj.mcpServers as Record<string, unknown>).atlas = {
      type: 'http',
      url: `${SERVER_URL}/api/mcp`,
      headers: { Authorization: `Bearer ${token.value.trim()}` },
    };
  }
  return JSON.stringify(obj, null, 2);
});

const mcpCliCmd = computed(() =>
  token.value.trim()
    ? `mcp add --transport http atlas ${SERVER_URL}/api/mcp`
    : `mcp add --transport http atlas ${SERVER_URL}/api/mcp`,
);

async function copy(text: string, key: string) {
  await navigator.clipboard.writeText(text);
  copied.value = key;
  setTimeout(() => { copied.value = null; }, 2000);
}
</script>

<template>
  <div class="w-[480px] bg-bg-elevated border border-border-primary rounded-lg shadow-2xl overflow-hidden animate-in fade-in slide-in-from-top-2 duration-200">
    <div class="h-10 px-4 flex items-center justify-between border-b border-border-primary bg-bg-sidebar">
      <div class="flex items-center gap-2">
        <svg class="w-3.5 h-3.5 text-accent-blue" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4" />
        </svg>
        <span class="text-[10px] font-black uppercase tracking-widest text-text-secondary">
          {{ $t('mcpConfig.title') }}
        </span>
      </div>
      <div class="flex items-center gap-2">
        <button
          class="flex items-center gap-1 text-[8px] font-black uppercase tracking-wider px-2 py-0.5 rounded border transition-colors"
          :class="collapsed
            ? 'border-accent-blue/40 text-accent-blue bg-accent-blue/10 hover:bg-accent-blue/20'
            : 'border-border-primary text-text-tertiary hover:text-text-primary hover:bg-white/5'"
          @click="collapsed = !collapsed"
        >
          <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
            <path v-if="collapsed" stroke-linecap="round" stroke-linejoin="round" d="M5 15l7-7 7 7" />
            <path v-else stroke-linecap="round" stroke-linejoin="round" d="M19 9l-7 7-7-7" />
          </svg>
          {{ collapsed ? 'Expand' : 'Collapse' }}
        </button>
      </div>
    </div>

    <div v-if="!collapsed" class="p-4 space-y-4">
      <div class="space-y-1.5">
        <label class="text-[9px] font-black uppercase tracking-widest text-text-tertiary">
          {{ $t('mcpConfig.tokenLabel') }}
        </label>
        <input
          v-model="token"
          type="password"
          class="w-full bg-bg-sidebar border border-border-primary rounded px-3 py-1.5 text-[11px] text-text-primary placeholder-text-tertiary focus:outline-none focus:border-accent-blue font-mono"
          :placeholder="$t('mcpConfig.tokenPlaceholder')"
        />
        <p class="text-[9px] text-text-tertiary opacity-50">{{ $t('mcpConfig.tokenHint') }}</p>
      </div>

      <div class="space-y-1.5">
        <div class="flex items-center justify-between">
          <label class="text-[9px] font-black uppercase tracking-widest text-text-tertiary">
            .mcp.json
          </label>
          <button
            class="text-[9px] font-black uppercase tracking-wider px-2 py-0.5 rounded transition-colors"
            :class="copied === 'json' ? 'text-accent-green' : 'text-accent-blue hover:text-accent-blue/80'"
            @click="copy(dotMcpJson, 'json')"
          >
            {{ copied === 'json' ? $t('mcpConfig.copied') : $t('mcpConfig.copy') }}
          </button>
        </div>
        <pre class="bg-bg-sidebar border border-border-primary rounded px-3 py-2 text-[10px] text-accent-green font-mono overflow-x-auto">{{ dotMcpJson }}</pre>
        <p class="text-[9px] text-text-tertiary opacity-50">{{ $t('mcpConfig.jsonHint') }}</p>
      </div>

      <div class="space-y-1.5">
        <div class="flex items-center justify-between">
          <label class="text-[9px] font-black uppercase tracking-widest text-text-tertiary">
            {{ $t('mcpConfig.cliLabel') }}
          </label>
          <button
            class="text-[9px] font-black uppercase tracking-wider px-2 py-0.5 rounded transition-colors"
            :class="copied === 'cli' ? 'text-accent-green' : 'text-accent-blue hover:text-accent-blue/80'"
            @click="copy(mcpCliCmd, 'cli')"
          >
            {{ copied === 'cli' ? $t('mcpConfig.copied') : $t('mcpConfig.copy') }}
          </button>
        </div>
        <pre class="bg-bg-sidebar border border-border-primary rounded px-3 py-2 text-[10px] text-yellow-400 font-mono overflow-x-auto">{{ mcpCliCmd }}</pre>
      </div>

      <div class="pt-2 border-t border-border-primary">
        <p class="text-[9px] text-text-tertiary opacity-50 leading-relaxed">
          {{ $t('mcpConfig.footer') }}
        </p>
      </div>
    </div>
  </div>
</template>
