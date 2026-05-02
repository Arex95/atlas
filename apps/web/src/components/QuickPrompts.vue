<script setup lang="ts">
import { ref, computed, nextTick } from 'vue';
import { QUICK_PROMPTS, type QuickPrompt } from '@/data/quick-prompts';

const emit = defineEmits<{
  select: [text: string]
}>();

const open = ref(false);
const collapsed = ref(false);
const activeCategory = ref<string>('all');
const btnRef = ref<HTMLElement | null>(null);
const panelStyle = ref<Record<string, string>>({});

const categories = ['all', 'atlas', 'context', 'workflow', 'debug'] as const;

const filtered = computed(() =>
  activeCategory.value === 'all'
    ? QUICK_PROMPTS
    : QUICK_PROMPTS.filter((p) => p.category === activeCategory.value),
);

const categoryColor: Record<string, string> = {
  atlas: 'text-accent-blue border-accent-blue/30 bg-accent-blue/10',
  context: 'text-purple-400 border-purple-400/30 bg-purple-400/10',
  workflow: 'text-accent-green border-accent-green/30 bg-accent-green/10',
  debug: 'text-yellow-400 border-yellow-400/30 bg-yellow-400/10',
};

async function toggleOpen() {
  open.value = !open.value;
  if (open.value) {
    collapsed.value = false;
    await nextTick();
    const rect = btnRef.value?.getBoundingClientRect();
    if (rect) {
      panelStyle.value = {
        position: 'fixed',
        bottom: `${window.innerHeight - rect.top + 8}px`,
        left: `${rect.left}px`,
        zIndex: '9999',
      };
    }
  }
}

function pick(prompt: QuickPrompt) {
  emit('select', prompt.text);
  open.value = false;
}
</script>

<template>
  <div class="relative">
    <button
      ref="btnRef"
      class="flex items-center gap-1.5 px-2 py-1 rounded text-[10px] font-black uppercase tracking-wider transition-colors"
      :class="open ? 'bg-accent-blue/20 text-accent-blue' : 'text-text-tertiary hover:text-text-secondary hover:bg-white/5'"
      :title="$t('quickPrompts.tooltip')"
      @click="toggleOpen"
    >
      <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
        <path stroke-linecap="round" stroke-linejoin="round" d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z" />
      </svg>
      {{ $t('quickPrompts.button') }}
    </button>
  </div>

  <Teleport to="body">
    <Transition name="prompt-menu">
      <div
        v-if="open"
        :style="panelStyle"
        class="w-96 bg-bg-elevated border border-border-primary rounded-lg shadow-2xl overflow-hidden"
      >
        <div class="h-8 px-3 flex items-center justify-between border-b border-border-primary bg-bg-sidebar">
          <span class="text-[9px] font-black uppercase tracking-widest text-text-tertiary">{{ $t('quickPrompts.title') }}</span>
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
            <button class="text-text-tertiary hover:text-text-primary transition-colors p-1 rounded hover:bg-white/5" @click="open = false">
              <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M6 18L18 6M6 6l12 12" /></svg>
            </button>
          </div>
        </div>

        <template v-if="!collapsed">
          <div class="flex gap-1 px-3 py-2 border-b border-border-primary">
            <button
              v-for="cat in categories"
              :key="cat"
              class="text-[8px] font-black uppercase tracking-wider px-2 py-0.5 rounded transition-colors"
              :class="activeCategory === cat ? 'bg-white/10 text-text-primary' : 'text-text-tertiary hover:text-text-secondary'"
              @click="activeCategory = cat"
            >
              {{ cat }}
            </button>
          </div>

          <div class="max-h-72 overflow-y-auto scrollbar-hide">
            <button
              v-for="prompt in filtered"
              :key="prompt.id"
              class="w-full text-left px-3 py-2.5 flex items-start gap-3 hover:bg-white/[0.03] transition-colors border-b border-border-primary/50 last:border-0 group"
              @click="pick(prompt)"
            >
              <span
                class="flex-none text-[8px] font-black uppercase tracking-wider px-1.5 py-0.5 rounded border mt-0.5"
                :class="categoryColor[prompt.category]"
              >
                {{ prompt.category }}
              </span>
              <div class="flex-1 min-w-0">
                <p class="text-[11px] font-bold text-text-primary group-hover:text-white transition-colors">{{ prompt.label }}</p>
                <p class="text-[9px] text-text-tertiary opacity-70 mt-0.5">{{ prompt.description }}</p>
              </div>
              <svg class="flex-none w-3 h-3 text-text-tertiary opacity-0 group-hover:opacity-100 transition-opacity mt-1" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path d="M9 5l7 7-7 7" />
              </svg>
            </button>
          </div>

          <div class="px-3 py-2 border-t border-border-primary bg-bg-sidebar/50">
            <p class="text-[8px] text-text-tertiary opacity-50">{{ $t('quickPrompts.hint') }}</p>
          </div>
        </template>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.scrollbar-hide::-webkit-scrollbar { display: none; }
.scrollbar-hide { -ms-overflow-style: none; scrollbar-width: none; }

.prompt-menu-enter-active,
.prompt-menu-leave-active {
  transition: all 0.15s ease;
}
.prompt-menu-enter-from,
.prompt-menu-leave-to {
  opacity: 0;
  transform: translateY(4px);
}
</style>
