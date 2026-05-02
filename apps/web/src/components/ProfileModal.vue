<script setup lang="ts">
import { ref, watch } from 'vue';
import { useProfileStore } from '@/stores/profile';

const emit = defineEmits<{ close: [] }>();

const store = useProfileStore();

const form = ref({ ...store.profile });
const saving = ref(false);

watch(() => store.profile, (p) => { form.value = { ...p }; }, { deep: true });

async function save() {
  saving.value = true;
  try {
    await store.update(form.value);
    emit('close');
  } finally {
    saving.value = false;
  }
}

const colors = ['#3b82f6', '#10b981', '#8b5cf6', '#f59e0b', '#ef4444', '#06b6d4', '#ec4899', '#84cc16'];
</script>

<template>
  <div class="fixed inset-0 z-[9999] flex items-center justify-center bg-black/60 backdrop-blur-sm" @click.self="emit('close')">
    <div class="w-[480px] bg-bg-elevated border border-border-primary rounded-lg shadow-2xl overflow-hidden">

      <div class="h-10 px-4 flex items-center justify-between border-b border-border-primary bg-bg-sidebar">
        <span class="text-[10px] font-black uppercase tracking-widest text-text-secondary">{{ $t('profile.title') }}</span>
        <button class="text-text-tertiary hover:text-text-primary transition-colors p-1 rounded hover:bg-white/5" @click="emit('close')">
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M6 18L18 6M6 6l12 12"/></svg>
        </button>
      </div>

      <div class="p-6 space-y-5">

        <!-- avatar preview -->
        <div class="flex items-center gap-4">
          <div
            class="w-14 h-14 rounded-lg flex items-center justify-center text-[18px] font-black text-white border border-white/10 flex-none"
            :style="{ backgroundColor: form.avatarColor }"
          >
            {{ store.initials }}
          </div>
          <div class="flex flex-col gap-1.5 flex-1">
            <p class="text-[9px] font-black uppercase tracking-widest text-text-tertiary opacity-50">{{ $t('profile.avatarColor') }}</p>
            <div class="flex gap-2 flex-wrap">
              <button
                v-for="c in colors"
                :key="c"
                class="w-5 h-5 rounded transition-transform hover:scale-110"
                :class="form.avatarColor === c ? 'ring-2 ring-white ring-offset-1 ring-offset-bg-elevated' : ''"
                :style="{ backgroundColor: c }"
                @click="form.avatarColor = c"
              />
            </div>
          </div>
        </div>

        <div class="grid grid-cols-2 gap-4">
          <div class="space-y-1.5">
            <label class="text-[9px] font-black uppercase tracking-widest text-text-tertiary">{{ $t('profile.name') }}</label>
            <input
              v-model="form.name"
              class="w-full bg-bg-sidebar border border-border-primary rounded px-3 py-1.5 text-[12px] text-text-primary placeholder-text-tertiary focus:outline-none focus:border-accent-blue"
              :placeholder="$t('profile.namePlaceholder')"
            />
          </div>
          <div class="space-y-1.5">
            <label class="text-[9px] font-black uppercase tracking-widest text-text-tertiary">{{ $t('profile.title2') }}</label>
            <input
              v-model="form.title"
              class="w-full bg-bg-sidebar border border-border-primary rounded px-3 py-1.5 text-[12px] text-text-primary placeholder-text-tertiary focus:outline-none focus:border-accent-blue"
              :placeholder="$t('profile.titlePlaceholder')"
            />
          </div>
        </div>

        <div class="space-y-1.5">
          <label class="text-[9px] font-black uppercase tracking-widest text-text-tertiary">{{ $t('profile.email') }}</label>
          <input
            v-model="form.email"
            type="email"
            class="w-full bg-bg-sidebar border border-border-primary rounded px-3 py-1.5 text-[12px] text-text-primary placeholder-text-tertiary focus:outline-none focus:border-accent-blue font-mono"
            placeholder="dev@example.com"
          />
        </div>

        <div class="space-y-1.5">
          <label class="text-[9px] font-black uppercase tracking-widest text-text-tertiary">{{ $t('profile.github') }}</label>
          <div class="flex items-center gap-2">
            <span class="text-[11px] text-text-tertiary font-mono flex-none">github.com/</span>
            <input
              v-model="form.github"
              class="flex-1 bg-bg-sidebar border border-border-primary rounded px-3 py-1.5 text-[12px] text-text-primary placeholder-text-tertiary focus:outline-none focus:border-accent-blue font-mono"
              placeholder="username"
            />
          </div>
        </div>

        <div class="space-y-1.5">
          <label class="text-[9px] font-black uppercase tracking-widest text-text-tertiary">{{ $t('profile.website') }}</label>
          <input
            v-model="form.website"
            type="url"
            class="w-full bg-bg-sidebar border border-border-primary rounded px-3 py-1.5 text-[12px] text-text-primary placeholder-text-tertiary focus:outline-none focus:border-accent-blue font-mono"
            placeholder="https://yoursite.com"
          />
        </div>

        <div class="flex gap-3 pt-2">
          <button
            class="flex-1 py-2 rounded text-[10px] font-black uppercase tracking-wider bg-accent-blue/20 text-accent-blue hover:bg-accent-blue/30 transition-colors disabled:opacity-40"
            :disabled="saving"
            @click="save"
          >
            {{ $t('profile.save') }}
          </button>
          <button
            class="px-5 py-2 rounded text-[10px] font-black uppercase tracking-wider bg-white/5 text-text-tertiary hover:text-text-primary transition-colors"
            @click="emit('close')"
          >
            {{ $t('profile.cancel') }}
          </button>
        </div>
      </div>

      <div v-if="form.github || form.website" class="px-6 py-3 border-t border-border-primary bg-bg-sidebar/30 flex items-center gap-4">
        <a v-if="form.github" :href="`https://github.com/${form.github}`" target="_blank"
          class="flex items-center gap-1.5 text-[9px] text-text-tertiary hover:text-white transition-colors font-mono">
          <svg class="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 24 24">
            <path d="M12 .297c-6.63 0-12 5.373-12 12 0 5.303 3.438 9.8 8.205 11.385.6.113.82-.258.82-.577 0-.285-.01-1.04-.015-2.04-3.338.724-4.042-1.61-4.042-1.61C4.422 18.07 3.633 17.7 3.633 17.7c-1.087-.744.084-.729.084-.729 1.205.084 1.838 1.236 1.838 1.236 1.07 1.835 2.809 1.305 3.495.998.108-.776.417-1.305.76-1.605-2.665-.3-5.466-1.332-5.466-5.93 0-1.31.465-2.38 1.235-3.22-.135-.303-.54-1.523.105-3.176 0 0 1.005-.322 3.3 1.23.96-.267 1.98-.399 3-.405 1.02.006 2.04.138 3 .405 2.28-1.552 3.285-1.23 3.285-1.23.645 1.653.24 2.873.12 3.176.765.84 1.23 1.91 1.23 3.22 0 4.61-2.805 5.625-5.475 5.92.42.36.81 1.096.81 2.22 0 1.606-.015 2.896-.015 3.286 0 .315.21.69.825.57C20.565 22.092 24 17.592 24 12.297c0-6.627-5.373-12-12-12"/>
          </svg>
          {{ form.github }}
        </a>
        <a v-if="form.website" :href="form.website" target="_blank"
          class="flex items-center gap-1.5 text-[9px] text-text-tertiary hover:text-white transition-colors font-mono">
          <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path d="M21 12a9 9 0 01-9 9m9-9a9 9 0 00-9-9m9 9H3m9 9a9 9 0 01-9-9m9 9c1.657 0 3-4.03 3-9s-1.343-9-3-9m0 18c-1.657 0-3-4.03-3-9s1.343-9 3-9"/>
          </svg>
          {{ form.website.replace(/^https?:\/\//, '') }}
        </a>
      </div>
    </div>
  </div>
</template>
