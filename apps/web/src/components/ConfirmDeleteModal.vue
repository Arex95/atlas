<script setup lang="ts">
import { ref, computed } from 'vue';

const props = defineProps<{
  name: string;
  id: string;
  // legacy compat
  sessionName?: string;
  sessionId?: string;
}>();

const emit = defineEmits<{
  close: [];
  confirm: [id: string];
}>();

const confirmInput = ref('');
const targetName = computed(() => props.name || props.sessionName || '');
const targetId = computed(() => props.id || props.sessionId || '');
const isConfirmed = computed(() => confirmInput.value === targetName.value);
</script>

<template>
  <div class="fixed inset-0 z-[100] flex items-center justify-center p-6 bg-black/80 backdrop-blur-sm animate-in fade-in duration-200">
    <div class="bg-bg-sidebar border border-accent-red/30 rounded-xl shadow-2xl max-w-md w-full overflow-hidden animate-in zoom-in-95 duration-200">

      <div class="bg-accent-red/10 px-6 py-5 border-b border-accent-red/20 flex items-center gap-4">
        <div class="w-12 h-12 rounded-full bg-accent-red/20 flex items-center justify-center text-accent-red">
          <svg class="w-7 h-7" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
          </svg>
        </div>
        <div>
          <h3 class="text-sm font-black text-white uppercase tracking-widest">{{ $t('confirmDelete.title') }}</h3>
          <p class="text-[10px] text-accent-red font-bold uppercase opacity-80">{{ $t('confirmDelete.warning') }}</p>
        </div>
      </div>

      
      <div class="p-6 space-y-6">
        <div class="space-y-2">
          <p class="text-[12px] text-text-secondary leading-relaxed">
            {{ $t('confirmDelete.body') }} <span class="text-white font-bold">{{ targetName }}</span>.
            {{ $t('confirmDelete.detail') }}
          </p>
        </div>

        <div class="space-y-3">
          <label class="text-[10px] font-bold text-text-tertiary uppercase tracking-widest">
            {{ $t('confirmDelete.confirmLabel') }}
          </label>
          <input
            v-model="confirmInput"
            type="text"
            :placeholder="targetName"
            class="w-full bg-black/40 border border-white/5 rounded-lg px-4 py-3 text-sm text-white placeholder:text-white/10 focus:outline-none focus:border-accent-red/50 transition-all font-mono"
            @keyup.enter="isConfirmed && emit('confirm', targetId)"
          />
        </div>

        
        <div class="flex gap-3">
          <button 
            @click="emit('close')"
            class="flex-1 px-4 py-3 bg-white/5 hover:bg-white/10 text-white rounded-lg font-bold text-[11px] uppercase tracking-widest transition-all border border-white/5"
          >
            {{ $t('confirmDelete.cancel') }}
          </button>
          <button
            @click="emit('confirm', targetId)"
            :disabled="!isConfirmed"
            class="flex-1 px-4 py-3 bg-accent-red disabled:opacity-20 disabled:cursor-not-allowed hover:bg-accent-red/90 text-white rounded-lg font-bold text-[11px] uppercase tracking-widest transition-all shadow-lg shadow-accent-red/20 border border-accent-red/20"
          >
            {{ $t('confirmDelete.terminate') }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
