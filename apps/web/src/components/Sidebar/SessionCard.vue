<script setup lang="ts">
import { ref } from 'vue';
import { useI18n } from 'vue-i18n';
import type { AISession } from '@atlas/domain';
import { AIProvider } from '@atlas/domain';
import { formatPath } from '@/utils/path';

const { t } = useI18n();

const props = defineProps<{
  tab: AISession;
  isActive: boolean;
}>();

const emit = defineEmits<{
  select: [id: string];
  close: [id: string];
  save: [payload: { customName: string, customDescription?: string, color?: string, icon?: string }];
}>();

const isSaving = ref(false);
const saveForm = ref({
  name: props.tab.customName || (props.tab.model === AIProvider.Bash ? t('sessionCard.localTerminal') : props.tab.model),
  description: props.tab.customDescription || '',
  color: props.tab.color || '#3b82f6',
  icon: props.tab.icon || 'terminal'
});

async function handleSave() {
  emit('save', {
    customName: saveForm.value.name,
    customDescription: saveForm.value.description,
    color: saveForm.value.color,
    icon: saveForm.value.icon
  });
  isSaving.value = false;
}

const formatDate = (dateStr: string) => {
  const date = new Date(dateStr);
  return date.toLocaleDateString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
};

</script>

<template>
  <div
    @click="emit('select', tab.id)"
    :class="[
      'group relative px-4 py-3 transition-all cursor-pointer border-l-2',
      isActive ? 'bg-bg-elevated border-accent-green' : 'border-transparent hover:bg-bg-elevated-2'
    ]"
  >

    <template v-if="tab.isSaved">
      <div class="flex items-start justify-between gap-3 mb-1 pr-6">
        <div class="flex flex-col min-w-0">
          <div class="flex items-center gap-2">
            <div v-if="tab.color" class="w-2 h-2 rounded-full" :style="{ backgroundColor: tab.color }"></div>
            <span :class="['text-[13px] font-bold truncate', isActive ? 'text-white' : 'text-text-secondary']">
              {{ tab.customName || tab.model }}
            </span>
          </div>
          <span class="text-[10px] text-text-tertiary truncate">{{ formatPath(tab.workingDirectory) }}</span>
        </div>
        <div v-if="tab.git" class="flex flex-col items-end shrink-0">
           <span class="text-[10px] text-accent-blue font-bold tracking-tight"> {{ tab.git.branch }}</span>
           <div v-if="tab.git.hasChanges" class="flex gap-1 text-[9px] font-black">
              <span v-if="tab.git.insertions > 0" class="text-accent-green">+{{ tab.git.insertions }}</span>
              <span v-if="tab.git.deletions > 0" class="text-accent-red">-{{ tab.git.deletions }}</span>
           </div>
        </div>
      </div>

      <div v-if="isActive" class="mt-3 pt-3 border-t border-white/5 space-y-2">
         <p v-if="tab.customDescription" class="text-[10px] text-text-tertiary italic leading-relaxed mb-2">{{ tab.customDescription }}</p>
         <div class="grid grid-cols-2 gap-2 text-[9px] uppercase tracking-widest font-black text-text-tertiary">
            <div>
               <span class="block text-white/40 mb-0.5">{{ $t('sessionCard.started') }}</span>
               <span class="text-text-secondary">{{ formatDate(tab.startedAt) }}</span>
            </div>
            <div>
               <span class="block text-white/40 mb-0.5">{{ $t('sessionCard.engine') }}</span>
               <span class="text-text-secondary">{{ tab.provider.toUpperCase() }}</span>
            </div>
         </div>
      </div>
    </template>

    <template v-else>
      <div class="flex items-center gap-2 pr-6">
        <div class="w-2 h-2 rounded-full bg-text-tertiary/30"></div>
        <span class="text-[12px] text-text-tertiary group-hover:text-text-secondary truncate">
          {{ tab.model === AIProvider.Bash ? $t('sessionCard.unnamedTerminal') : $t('sessionCard.draftSession') }}
        </span>
        <span class="text-[8px] border border-text-tertiary/20 px-1 py-0.5 text-text-tertiary uppercase font-black shrink-0">{{ $t('sessionCard.draft') }}</span>
      </div>

      <div v-if="isActive" class="mt-3 flex flex-col gap-2">
         <div class="text-[9px] text-text-tertiary truncate mb-1 opacity-60">
           {{ formatPath(tab.workingDirectory) }}
         </div>

         
         <div v-if="isSaving" class="bg-black/40 p-3 border border-border-primary space-y-3" @click.stop>
            <input v-model="saveForm.name" :placeholder="$t('sessionCard.namePlaceholder')" class="w-full bg-bg-primary border border-border-primary px-2 py-1.5 text-[11px] text-white focus:outline-none focus:border-accent-blue" />
            <textarea v-model="saveForm.description" :placeholder="$t('sessionCard.descriptionPlaceholder')" class="w-full bg-bg-primary border border-border-primary px-2 py-1.5 text-[10px] text-text-secondary focus:outline-none focus:border-accent-blue h-12"></textarea>
            <div class="flex gap-2">
               <button @click="handleSave" class="flex-1 bg-accent-blue text-white text-[10px] font-bold py-1.5 hover:opacity-90">{{ $t('sessionCard.saveButton') }}</button>
               <button @click="isSaving = false" class="px-2 text-text-tertiary hover:text-white text-[10px]">{{ $t('sessionCard.cancel') }}</button>
            </div>
         </div>
         <button v-else @click.stop="isSaving = true" class="w-full border border-border-primary text-text-secondary hover:text-white py-1.5 text-[10px] font-bold transition-all">
           {{ $t('sessionCard.saveToWorkspace') }}
         </button>
      </div>
    </template>

    <button @click.stop="emit('close', tab.id)" class="absolute top-2 right-2 opacity-0 group-hover:opacity-100 p-1 hover:text-white transition-all">
      <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5"><path d="M6 18L18 6M6 6l12 12" /></svg>
    </button>
  </div>
</template>
