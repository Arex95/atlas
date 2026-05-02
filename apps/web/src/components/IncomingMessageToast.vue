<script setup lang="ts">
defineProps<{
  from: string;
  content: string;
  isAgent?: boolean;
}>();
</script>

<template>
  <Transition name="slide-up">
    <div class="absolute bottom-6 right-6 max-w-sm bg-bg-sidebar/80 backdrop-blur-xl border border-accent-blue/30 rounded-lg p-4 shadow-2xl flex flex-col gap-1 z-50 pointer-events-none">
      <div class="flex items-center gap-2">
        <div class="w-2 h-2 rounded-full bg-accent-blue animate-pulse" />
        <span class="text-[10px] text-accent-blue font-bold uppercase tracking-widest">{{ $t('terminal.incomingMessage') }}</span>
      </div>
      <div class="text-[11px] text-text-secondary font-bold">{{ $t('terminal.from') }}: {{ from }}</div>
      <div class="text-[12px] text-white/90 leading-relaxed italic">"{{ content }}"</div>
    </div>
  </Transition>
</template>

<style scoped>
.slide-up-enter-active,
.slide-up-leave-active {
  transition: all 0.4s cubic-bezier(0.16, 1, 0.3, 1);
}
.slide-up-enter-from {
  opacity: 0;
  transform: translateY(20px) scale(0.95);
}
.slide-up-leave-to {
  opacity: 0;
  transform: translateY(10px) scale(0.98);
}
</style>
