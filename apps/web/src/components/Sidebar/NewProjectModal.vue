<script setup lang="ts">
import { ref, watch } from 'vue';
import { DEFAULT_PROJECT_COLOR } from '@/utils/colors';

const emit = defineEmits<{
  create: [payload: { name: string; slug: string; rootPath: string; indexPath?: string; description?: string; color?: string }]
  close: []
}>();

const name = ref('');
const rootPath = ref('');
const description = ref('');
const selectedColor = ref<string>(DEFAULT_PROJECT_COLOR);

const slug = ref('');
watch(name, (val) => {
  slug.value = val.toLowerCase().replace(/\s+/g, '-').replace(/[^a-z0-9-]/g, '');
});

function submit() {
  if (!name.value.trim() || !rootPath.value.trim()) return;
  const cleanRoot = rootPath.value.trim().replace(/\/$/, '');
  emit('create', {
    name: name.value.trim(),
    slug: slug.value || name.value.toLowerCase().replace(/\s+/g, '-'),
    rootPath: cleanRoot,
    indexPath: `${cleanRoot}/PROJECT_INDEX.md`,
    description: description.value.trim(),
    color: selectedColor.value,
  });
  name.value = '';
  rootPath.value = '';
  description.value = '';
}
</script>

<template>

  <Teleport to="body">
    <div
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm"
      @click.self="emit('close')"
    >
      <div class="w-full max-w-md bg-bg-sidebar border border-border-primary shadow-2xl font-mono">

        <div class="flex items-center justify-between px-5 py-4 border-b border-border-primary">
          <div class="flex items-center gap-2">
            <svg class="w-4 h-4 text-accent-blue" fill="currentColor" viewBox="0 0 20 20">
              <path d="M2 6a2 2 0 012-2h4l2 2h4a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z"/>
            </svg>
            <span class="text-[13px] font-bold text-white uppercase tracking-widest">{{ $t('newProjectModal.title') }}</span>
          </div>
          <button @click="emit('close')" class="text-text-tertiary hover:text-white transition-colors">
            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
              <path d="M6 18L18 6M6 6l12 12"/>
            </svg>
          </button>
        </div>

        <div class="p-5 space-y-4">

          <div>
            <label class="block text-[10px] font-bold text-text-tertiary uppercase tracking-widest mb-1.5">{{ $t('newProjectModal.nameLabel') }}</label>
            <input
              v-model="name"
              @keyup.enter="submit"
              :placeholder="$t('newProjectModal.namePlaceholder')"
              autofocus
              class="w-full bg-bg-primary border border-border-primary px-3 py-2 text-[13px] text-white placeholder:text-text-tertiary/50 focus:outline-none focus:border-accent-blue transition-colors"
            />
            <div v-if="slug" class="mt-1 text-[10px] text-text-tertiary">
              {{ $t('newProjectModal.slugPrefix') }} <span class="text-accent-blue">{{ slug }}</span>
            </div>
          </div>

          <div>
            <label class="block text-[10px] font-bold text-text-tertiary uppercase tracking-widest mb-1.5">{{ $t('newProjectModal.rootPathLabel') }}</label>
            <input
              v-model="rootPath"
              @keyup.enter="submit"
              :placeholder="$t('newProjectModal.rootPathPlaceholder')"
              class="w-full bg-bg-primary border border-border-primary px-3 py-2 text-[13px] text-white font-mono placeholder:text-text-tertiary/50 focus:outline-none focus:border-accent-blue transition-colors"
            />
          </div>

          <div>
            <label class="block text-[10px] font-bold text-text-tertiary uppercase tracking-widest mb-1.5">{{ $t('newProjectModal.descriptionLabel') }}</label>
            <textarea
              v-model="description"
              :placeholder="$t('newProjectModal.descriptionPlaceholder')"
              rows="2"
              class="w-full bg-bg-primary border border-border-primary px-3 py-2 text-[12px] text-text-secondary placeholder:text-text-tertiary/50 focus:outline-none focus:border-accent-blue transition-colors resize-none"
            />
          </div>

          <div>
            <label class="block text-[10px] font-bold text-text-tertiary uppercase tracking-widest mb-2">{{ $t('newProjectModal.colorLabel') }}</label>
            <div class="flex items-center gap-4">
              <input
                type="color"
                v-model="selectedColor"
                class="w-10 h-10 bg-bg-primary border border-border-primary rounded cursor-pointer transition-transform hover:scale-105"
              />
              <div class="flex flex-col">
                <span class="text-[11px] text-white font-mono uppercase">{{ selectedColor }}</span>
                <span class="text-[9px] text-text-tertiary">{{ $t('newProjectModal.colorHint') }}</span>
              </div>
            </div>
          </div>
        </div>

        <div class="flex items-center gap-2 px-5 py-4 border-t border-border-primary">
          <button
            @click="submit"
            :disabled="!name.trim() || !rootPath.trim()"
            class="flex-1 bg-white text-black text-[11px] font-bold uppercase tracking-widest py-2.5 hover:bg-opacity-90 disabled:opacity-30 disabled:cursor-not-allowed transition-all"
          >
            {{ $t('newProjectModal.create') }}
          </button>
          <button
            @click="emit('close')"
            class="px-4 py-2.5 border border-border-primary text-text-tertiary hover:text-white text-[11px] font-bold uppercase tracking-widest transition-colors"
          >
            {{ $t('newProjectModal.cancel') }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>
