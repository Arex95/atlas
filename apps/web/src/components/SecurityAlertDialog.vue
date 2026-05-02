<script setup lang="ts">
defineProps<{ command: string }>();
defineEmits<{ approve: []; cancel: [] }>();
</script>

<template>
  <div class="absolute inset-0 z-50 flex items-center justify-center p-6 bg-black/60 backdrop-blur-sm">
    <div class="bg-bg-sidebar border border-accent-red/30 rounded-lg shadow-2xl max-w-md w-full overflow-hidden animate-in fade-in zoom-in duration-200">
      <div class="bg-accent-red/10 px-6 py-4 border-b border-accent-red/20 flex items-center gap-3">
        <div class="w-10 h-10 rounded-full bg-accent-red/20 flex items-center justify-center text-accent-red">
          <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
          </svg>
        </div>
        <div>
          <h3 class="text-[13px] font-black text-white uppercase tracking-widest">{{ $t('terminal.security.title') }}</h3>
          <p class="text-[10px] text-accent-red font-bold uppercase opacity-80">{{ $t('terminal.security.subtitle') }}</p>
        </div>
      </div>

      <div class="p-6 space-y-4">
        <p class="text-[12px] text-text-secondary leading-relaxed">
          {{ $t('terminal.security.body') }}
        </p>

        <div class="bg-black/40 p-4 rounded border border-white/5 font-mono text-[11px] break-all text-white">
          <span class="text-text-tertiary mr-2">$</span> {{ command }}
        </div>

        <div class="flex gap-3 pt-2">
          <button
            @click="$emit('cancel')"
            class="flex-1 px-4 py-2.5 bg-white/5 hover:bg-white/10 text-white rounded font-bold text-[11px] uppercase tracking-widest transition-all"
          >
            {{ $t('terminal.security.cancel') }}
          </button>
          <button
            @click="$emit('approve')"
            class="flex-1 px-4 py-2.5 bg-accent-red hover:bg-accent-red/90 text-white rounded font-bold text-[11px] uppercase tracking-widest transition-all shadow-lg shadow-accent-red/20"
          >
            {{ $t('terminal.security.runAnyway') }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
