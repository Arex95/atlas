<script setup lang="ts">
import { useToast } from '@/composables/useToast';

const { toasts, dismiss } = useToast();

const iconMap: Record<string, string> = {
  success: '✓',
  error: '✕',
  warning: '⚠',
  info: 'i',
  reminder: '⏰',
};

const colorMap: Record<string, string> = {
  success: 'border-accent-green text-accent-green',
  error: 'border-red-500 text-red-400',
  warning: 'border-yellow-500 text-yellow-400',
  info: 'border-accent-blue text-accent-blue',
  reminder: 'border-purple-500 text-purple-400',
};
</script>

<template>
  <Teleport to="body">
    <div class="fixed bottom-6 right-6 z-50 flex flex-col gap-2 pointer-events-none">
      <TransitionGroup
        name="toast"
        tag="div"
        class="flex flex-col gap-2"
      >
        <div
          v-for="toast in toasts"
          :key="toast.id"
          class="pointer-events-auto flex items-start gap-3 min-w-[280px] max-w-sm px-4 py-3 rounded-lg bg-bg-elevated border border-border-primary shadow-2xl"
          :class="colorMap[toast.kind]"
        >
          <div
            class="flex-none w-5 h-5 rounded-full flex items-center justify-center text-[10px] font-black border"
            :class="colorMap[toast.kind]"
          >
            {{ iconMap[toast.kind] }}
          </div>
          <div class="flex-1 min-w-0">
            <p v-if="toast.title" class="text-[10px] font-black uppercase tracking-wider mb-0.5">
              {{ toast.title }}
            </p>
            <p class="text-[11px] text-text-secondary leading-relaxed">{{ toast.message }}</p>
          </div>
          <button
            class="flex-none text-text-tertiary hover:text-text-primary transition-colors text-[14px] leading-none"
            @click="dismiss(toast.id)"
          >
            ×
          </button>
        </div>
      </TransitionGroup>
    </div>
  </Teleport>
</template>

<style scoped>
.toast-enter-active,
.toast-leave-active {
  transition: all 0.25s ease;
}
.toast-enter-from {
  opacity: 0;
  transform: translateX(20px);
}
.toast-leave-to {
  opacity: 0;
  transform: translateX(20px);
}
</style>
