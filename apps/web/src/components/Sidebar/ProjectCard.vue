<script setup lang="ts">
import { ref } from 'vue';
import type { Project, AISession } from '@atlas/domain';
import { formatPath } from '@/utils/path';
import ConfirmDeleteModal from '@/components/ConfirmDeleteModal.vue';

const props = defineProps<{
  project: Project;
  isSelected: boolean;
  sessions: AISession[];
  activeTabId: string | null;
}>();

const emit = defineEmits<{
  select: [slug: string]
  openSession: [id: string]
  addSession: [slug: string]
  closeSession: [id: string]
  showDashboard: [slug: string]
  showSessionInfo: [id: string]
  deleteProject: [slug: string]
}>();

const isExpanded = ref(props.isSelected);
const showMoreInfo = ref(false);
const showDeleteModal = ref(false);

function toggle() {
  isExpanded.value = !isExpanded.value;
  emit('select', props.project.slug);
}

function onDeleteConfirmed() {
  showDeleteModal.value = false;
  emit('deleteProject', props.project.slug);
}
</script>

<template>
  <div class="px-2 mb-2">
    <div
      :class="[
        'group bg-bg-sidebar/40 border transition-all duration-200 overflow-hidden',
        isSelected ? 'border-border-primary ring-1 ring-white/5' : 'border-white/5 hover:border-white/10'
      ]"
      :style="{ borderLeftColor: project.color, borderLeftWidth: '3px' }"
    >

      <div
        @click="toggle"
        class="flex items-center gap-2 px-3 py-3 cursor-pointer"
      >

        <svg
          :class="['w-3 h-3 shrink-0 transition-transform duration-150', isExpanded ? 'rotate-90' : '']"
          fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5"
        >
          <path d="M9 18l6-6-6-6"/>
        </svg>

        <div class="flex flex-col min-w-0 flex-1">
          <span :class="['text-[12px] font-bold truncate', isSelected ? 'text-white' : 'text-text-secondary']">
            {{ project.name }}
          </span>
          <span v-if="!isExpanded" class="text-[9px] text-text-tertiary opacity-60 truncate">
            {{ formatPath(project.rootPath) }}
          </span>
        </div>

        <div class="flex items-center gap-1.5 opacity-0 group-hover:opacity-100 transition-opacity">

          <button
            @click.stop="emit('showDashboard', project.slug)"
            class="p-1 hover:text-white text-text-tertiary transition-colors"
            :title="$t('projectCard.viewDashboard')"
          >
            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
              <path d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z" />
            </svg>
          </button>

          
          <button
            @click.stop="emit('addSession', project.slug)"
            class="p-1 hover:text-accent-green text-text-tertiary transition-colors"
            :title="$t('projectCard.newTerminal')"
          >
            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
              <path d="M12 5v14M5 12h14"/>
            </svg>
          </button>

          <button
            @click.stop="showDeleteModal = true"
            class="p-1 hover:text-accent-red text-text-tertiary transition-colors"
            :title="$t('projectCard.deleteProject')"
          >
            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
            </svg>
          </button>
        </div>
      </div>

      <div v-if="isExpanded" class="px-3 pb-3">
        <button 
          @click.stop="showMoreInfo = !showMoreInfo"
          class="text-[9px] font-black uppercase tracking-widest text-text-tertiary hover:text-text-secondary flex items-center gap-1 mb-2"
        >
          <span>{{ showMoreInfo ? $t('projectCard.hideDetails') : $t('projectCard.showDetails') }}</span>
          <svg :class="['w-2.5 h-2.5 transition-transform', showMoreInfo ? 'rotate-180' : '']" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="3"><path d="M19 9l-7 7-7-7"/></svg>
        </button>

        <div v-if="showMoreInfo" class="space-y-2 mb-3 p-2 bg-black/20 rounded-sm border border-white/5">
          <div class="flex flex-col gap-0.5">
            <span class="text-[8px] uppercase text-white/30 font-bold">{{ $t('projectCard.path') }}</span>
            <span class="text-[9px] text-text-secondary font-mono truncate">{{ formatPath(project.rootPath) }}</span>
          </div>
          <div class="flex flex-col gap-0.5">
            <span class="text-[8px] uppercase text-white/30 font-bold">{{ $t('projectCard.description') }}</span>
            <span class="text-[9px] text-text-secondary leading-tight line-clamp-2">{{ project.description || $t('projectCard.noDescription') }}</span>
          </div>
        </div>

        <div class="space-y-[1px] border-t border-white/5 pt-2">
          <div v-if="sessions.length === 0" class="px-2 py-1.5 text-[10px] text-text-tertiary italic opacity-40">
            {{ $t('projectCard.noTerminals') }}
          </div>
          <div
            v-for="session in sessions"
            :key="session.id"
            @click.stop="emit('openSession', session.id)"
            :class="[
              'group/session flex items-center gap-2 px-2 py-1.5 cursor-pointer transition-colors rounded-sm',
              activeTabId === session.id
                ? 'bg-white/5 text-white'
                : 'text-text-tertiary hover:text-text-secondary hover:bg-white/[0.03]'
            ]"
          >
            <span :class="['w-1.5 h-1.5 rounded-full shrink-0', activeTabId === session.id ? 'bg-accent-green' : 'bg-text-tertiary/20']"/>
            <span class="text-[10px] truncate flex-1 font-bold">{{ session.customName || session.title || session.model }}</span>

            <div class="flex items-center gap-1 opacity-0 group-hover/session:opacity-100 transition-all">
              <button
                @click.stop="emit('showSessionInfo', session.id)"
                class="p-1 hover:text-white transition-colors"
                :title="$t('sessionCard.sessionInfo')"
              >
                <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                  <path d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                  <path d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z" />
                </svg>
              </button>
              <button
                @click.stop="emit('closeSession', session.id)"
                class="p-1 hover:text-accent-red transition-all"
                :title="$t('sessionCard.closeTerminal')"
              >
                <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5"><path d="M6 18L18 6M6 6l12 12"/></svg>
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>

  <Teleport to="body">
    <ConfirmDeleteModal
      v-if="showDeleteModal"
      :name="project.name"
      :id="project.slug"
      @close="showDeleteModal = false"
      @confirm="onDeleteConfirmed"
    />
  </Teleport>
</template>
