<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { useI18n } from 'vue-i18n';
import { api } from '@/api/client';
import { useToast } from '@/composables/useToast';

const { t: $t } = useI18n();
const toast = useToast();
type Tab = 'memory' | 'skills' | 'prompts';
const activeTab = ref<Tab>('memory');

// ── Memory ────────────────────────────────────────────────────────────────────
interface MemoryRow { id: string; key: string; value: string; description: string; updatedAt: string }
const memory = ref<MemoryRow[]>([]);
const newMemKey = ref('');
const newMemValue = ref('');
const newMemDesc = ref('');
const editingMem = ref<MemoryRow | null>(null);

async function fetchMemory() {
  memory.value = await api.get<MemoryRow[]>('/api/global/memory').catch(() => []);
}

async function saveMem() {
  const key = (editingMem.value?.key ?? newMemKey.value).trim();
  const value = (editingMem.value ? editingMem.value.value : newMemValue.value).trim();
  const description = (editingMem.value ? editingMem.value.description : newMemDesc.value).trim();
  if (!key || !value) return;
  await api.post('/api/global/memory', { key, value, description });
  editingMem.value = null;
  newMemKey.value = newMemValue.value = newMemDesc.value = '';
  fetchMemory();
}

async function deleteMem(key: string) {
  await api.delete(`/api/global/memory/${encodeURIComponent(key)}`);
  fetchMemory();
}

// ── Skills ────────────────────────────────────────────────────────────────────
interface SkillRow { id: string; name: string; description: string; trigger?: string; script: string; usageCount: number }
const skills = ref<SkillRow[]>([]);
const showSkillForm = ref(false);
const editingSkill = ref<SkillRow | null>(null);
const skillForm = ref({ name: '', description: '', trigger: '', script: '' });

async function fetchSkills() {
  skills.value = await api.get<SkillRow[]>('/api/global/skills').catch(() => []);
}

function startSkill(s?: SkillRow) {
  editingSkill.value = s ?? null;
  skillForm.value = s
    ? { name: s.name, description: s.description, trigger: s.trigger ?? '', script: s.script }
    : { name: '', description: '', trigger: '', script: '' };
  showSkillForm.value = true;
}

async function saveSkill() {
  if (!skillForm.value.name.trim()) return;
  if (editingSkill.value) {
    await api.patch(`/api/global/skills/${editingSkill.value.id}`, skillForm.value);
  } else {
    await api.post('/api/global/skills', skillForm.value);
  }
  showSkillForm.value = false;
  editingSkill.value = null;
  fetchSkills();
}

async function deleteSkill(id: string) {
  await api.delete(`/api/global/skills/${id}`);
  fetchSkills();
}

async function copyScript(text: string) {
  await navigator.clipboard.writeText(text);
  toast.show($t('globalContext.scriptCopied'), 'success');
}

// ── Prompts ───────────────────────────────────────────────────────────────────
interface PromptRow { id: string; title: string; content: string; updatedAt: string }
const prompts = ref<PromptRow[]>([]);
const showPromptForm = ref(false);
const editingPrompt = ref<PromptRow | null>(null);
const promptForm = ref({ title: '', content: '' });

async function fetchPrompts() {
  prompts.value = await api.get<PromptRow[]>('/api/global/prompts').catch(() => []);
}

function startPrompt(p?: PromptRow) {
  editingPrompt.value = p ?? null;
  promptForm.value = p ? { title: p.title, content: p.content } : { title: '', content: '' };
  showPromptForm.value = true;
}

async function savePrompt() {
  if (!promptForm.value.title.trim()) return;
  if (editingPrompt.value) {
    await api.patch(`/api/global/prompts/${editingPrompt.value.id}`, promptForm.value);
  } else {
    await api.post('/api/global/prompts', promptForm.value);
  }
  showPromptForm.value = false;
  editingPrompt.value = null;
  fetchPrompts();
}

async function deletePrompt(id: string) {
  await api.delete(`/api/global/prompts/${id}`);
  fetchPrompts();
}

async function copyPrompt(text: string) {
  await navigator.clipboard.writeText(text);
  toast.show($t('globalContext.promptCopied'), 'success');
}

onMounted(() => {
  fetchMemory();
  fetchSkills();
  fetchPrompts();
});

const tabs: { id: Tab; labelKey: string }[] = [
  { id: 'memory', labelKey: 'globalContext.tabMemory' },
  { id: 'skills', labelKey: 'globalContext.tabSkills' },
  { id: 'prompts', labelKey: 'globalContext.tabPrompts' },
];

const emit = defineEmits<{ close: [] }>();
</script>

<template>
  <Teleport to="body">
    <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm" @click.self="emit('close')" @keydown.esc="emit('close')">
      <div class="w-full max-w-4xl max-h-[90vh] overflow-y-auto mx-4 bg-bg-primary border border-border-primary shadow-2xl rounded-lg">
    <div class="bg-bg-sidebar/20 border border-border-primary overflow-hidden shadow-2xl">

      <!-- Header -->
      <div class="px-8 py-8 border-b border-border-primary bg-gradient-to-br from-bg-sidebar/40 to-transparent">
        <div class="flex items-center gap-4">
          <div class="p-3 border rounded-lg bg-accent-purple/10 border-accent-purple/30">
            <svg class="w-6 h-6 text-accent-purple" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M3.055 11H5a2 2 0 012 2v1a2 2 0 002 2 2 2 0 012 2v2.945M8 3.935V5.5A2.5 2.5 0 0010.5 8h.5a2 2 0 012 2 2 2 0 104 0 2 2 0 012-2h1.064M15 20.488V18a2 2 0 012-2h3.064" />
            </svg>
          </div>
          <div class="flex-1">
            <h1 class="text-3xl font-black text-white tracking-tighter uppercase">Global Context</h1>
            <p class="text-[11px] text-text-tertiary font-mono mt-1">Shared across all projects · Read-only for agents · Written from here</p>
          </div>
        </div>
      </div>

      <!-- Tabs -->
      <div class="flex border-b border-border-primary bg-black/10">
        <button
          v-for="tab in tabs"
          :key="tab.id"
          class="px-6 py-3 text-[10px] font-black uppercase tracking-widest transition-colors border-r border-border-primary last:border-0"
          :class="activeTab === tab.id
            ? 'text-white border-b-2 border-accent-purple -mb-px bg-bg-primary/30'
            : 'text-text-tertiary hover:text-text-secondary'"
          @click="activeTab = tab.id"
        >{{ $t(tab.labelKey) }}</button>
      </div>

      <div class="p-6">

        <!-- ── MEMORY ─────────────────────────────────────────────────────── -->
        <div v-if="activeTab === 'memory'" class="flex flex-col gap-4">
          <p class="text-[10px] text-text-tertiary font-mono opacity-70">
            Key/value pairs visible to every agent via <code class="bg-bg-sidebar px-1 rounded">global_list_memory</code> or <code class="bg-bg-sidebar px-1 rounded">atlas://global</code>
          </p>

          <!-- Add / edit form -->
          <div class="bg-bg-elevated/20 border border-border-primary rounded-lg p-4 flex flex-col gap-3">
            <p class="text-[9px] font-black uppercase tracking-widest text-accent-purple opacity-80">
              {{ editingMem ? $t('globalContext.editEntry') : $t('globalContext.newEntry') }}
            </p>
            <div class="grid grid-cols-2 gap-3">
              <div>
                <label class="block text-[9px] text-text-tertiary font-mono mb-1">{{ $t('globalContext.memoryKey') }}</label>
                <input
                  :value="editingMem ? editingMem.key : newMemKey"
                  @input="editingMem ? (editingMem.key = ($event.target as HTMLInputElement).value) : (newMemKey = ($event.target as HTMLInputElement).value)"
                  :disabled="!!editingMem"
                  class="w-full bg-bg-primary border border-border-primary px-3 py-2 text-[11px] font-mono text-text-primary outline-none focus:border-accent-purple disabled:opacity-50"
                  placeholder="e.g. USER_TIMEZONE"
                />
              </div>
              <div>
                <label class="block text-[9px] text-text-tertiary font-mono mb-1">{{ $t('globalContext.memoryValue') }}</label>
                <input
                  :value="editingMem ? editingMem.value : newMemValue"
                  @input="editingMem ? (editingMem.value = ($event.target as HTMLInputElement).value) : (newMemValue = ($event.target as HTMLInputElement).value)"
                  class="w-full bg-bg-primary border border-border-primary px-3 py-2 text-[11px] font-mono text-text-primary outline-none focus:border-accent-purple"
                  placeholder="e.g. America/Bogota"
                />
              </div>
            </div>
            <div>
              <label class="block text-[9px] text-text-tertiary font-mono mb-1">{{ $t('globalContext.memoryDescription') }}</label>
              <input
                :value="editingMem ? editingMem.description : newMemDesc"
                @input="editingMem ? (editingMem.description = ($event.target as HTMLInputElement).value) : (newMemDesc = ($event.target as HTMLInputElement).value)"
                class="w-full bg-bg-primary border border-border-primary px-3 py-2 text-[11px] font-mono text-text-primary outline-none focus:border-accent-purple"
                placeholder="What this value means"
              />
            </div>
            <div class="flex gap-2">
              <button
                class="px-4 py-2 bg-accent-purple text-white text-[10px] font-black uppercase tracking-widest hover:bg-accent-purple/80 transition-colors"
                @click="saveMem"
              >{{ $t('globalContext.save') }}</button>
              <button
                v-if="editingMem"
                class="px-4 py-2 border border-border-primary text-text-tertiary text-[10px] font-black uppercase tracking-widest hover:text-white transition-colors"
                @click="editingMem = null"
              >{{ $t('globalContext.cancel') }}</button>
            </div>
          </div>

          <!-- List -->
          <div v-if="memory.length === 0" class="py-8 text-center opacity-40">
            <p class="text-[11px] text-text-tertiary font-mono">{{ $t('globalContext.noMemory') }}</p>
          </div>
          <div v-else class="space-y-2">
            <div
              v-for="row in memory"
              :key="row.id"
              class="flex items-start gap-4 p-4 bg-bg-elevated/10 border border-border-primary rounded-lg group"
            >
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2 mb-1">
                  <span class="text-[11px] font-black font-mono text-accent-purple">{{ row.key }}</span>
                </div>
                <p class="text-[12px] font-mono text-text-primary break-all">{{ row.value }}</p>
                <p v-if="row.description" class="text-[9px] font-mono text-text-tertiary mt-1 opacity-60">{{ row.description }}</p>
              </div>
              <div class="flex gap-2 opacity-0 group-hover:opacity-100 transition-opacity flex-none">
                <button class="text-[9px] font-mono text-accent-blue hover:text-white transition-colors" @click="editingMem = { ...row }">{{ $t('globalContext.editAction') }}</button>
                <button class="text-[9px] font-mono text-accent-red hover:text-white transition-colors" @click="deleteMem(row.key)">{{ $t('globalContext.deleteAction') }}</button>
              </div>
            </div>
          </div>
        </div>

        <!-- ── SKILLS ─────────────────────────────────────────────────────── -->
        <div v-else-if="activeTab === 'skills'" class="flex flex-col gap-4">
          <div class="flex items-center justify-between">
            <p class="text-[10px] text-text-tertiary font-mono opacity-70">
              Scripts executable by any agent via <code class="bg-bg-sidebar px-1 rounded">run_skill</code>
            </p>
            <button
              class="px-4 py-2 bg-accent-purple text-white text-[10px] font-black uppercase tracking-widest hover:bg-accent-purple/80 transition-colors"
              @click="startSkill()"
            >{{ $t('globalContext.newSkillButton') }}</button>
          </div>

          <!-- Form -->
          <div v-if="showSkillForm" class="bg-bg-elevated/20 border border-border-primary rounded-lg p-4 flex flex-col gap-3">
            <p class="text-[9px] font-black uppercase tracking-widest text-accent-purple opacity-80">
              {{ editingSkill ? $t('globalContext.editSkill') : $t('globalContext.newGlobalSkill') }}
            </p>
            <div class="grid grid-cols-2 gap-3">
              <div>
                <label class="block text-[9px] text-text-tertiary font-mono mb-1">{{ $t('globalContext.skillName') }}</label>
                <input v-model="skillForm.name" class="w-full bg-bg-primary border border-border-primary px-3 py-2 text-[11px] font-mono text-text-primary outline-none focus:border-accent-purple" placeholder="e.g. run-tests" />
              </div>
              <div>
                <label class="block text-[9px] text-text-tertiary font-mono mb-1">{{ $t('globalContext.skillTrigger') }}</label>
                <input v-model="skillForm.trigger" class="w-full bg-bg-primary border border-border-primary px-3 py-2 text-[11px] font-mono text-text-primary outline-none focus:border-accent-purple" placeholder="e.g. @test" />
              </div>
            </div>
            <div>
              <label class="block text-[9px] text-text-tertiary font-mono mb-1">{{ $t('globalContext.skillDescription') }}</label>
              <input v-model="skillForm.description" class="w-full bg-bg-primary border border-border-primary px-3 py-2 text-[11px] font-mono text-text-primary outline-none focus:border-accent-purple" placeholder="What this skill does" />
            </div>
            <div>
              <label class="block text-[9px] text-text-tertiary font-mono mb-1">{{ $t('globalContext.skillScript') }}</label>
              <textarea v-model="skillForm.script" rows="5" class="w-full bg-bg-primary border border-border-primary px-3 py-2 text-[11px] font-mono text-text-primary outline-none focus:border-accent-purple resize-none" placeholder="#!/bin/bash&#10;echo 'hello'" />
            </div>
            <div class="flex gap-2">
              <button class="px-4 py-2 bg-accent-purple text-white text-[10px] font-black uppercase tracking-widest hover:bg-accent-purple/80 transition-colors" @click="saveSkill">{{ $t('globalContext.save') }}</button>
              <button class="px-4 py-2 border border-border-primary text-text-tertiary text-[10px] font-black uppercase tracking-widest hover:text-white transition-colors" @click="showSkillForm = false">{{ $t('globalContext.cancel') }}</button>
            </div>
          </div>

          <div v-if="skills.length === 0 && !showSkillForm" class="py-8 text-center opacity-40">
            <p class="text-[11px] text-text-tertiary font-mono">{{ $t('globalContext.noSkills') }}</p>
          </div>
          <div v-else class="space-y-2">
            <div
              v-for="skill in skills"
              :key="skill.id"
              class="p-4 bg-bg-elevated/10 border border-border-primary rounded-lg group"
            >
              <div class="flex items-start gap-4">
                <div class="flex-1 min-w-0">
                  <div class="flex items-center gap-2 mb-1">
                    <span class="text-[12px] font-black text-text-primary">{{ skill.name }}</span>
                    <span v-if="skill.trigger" class="text-[9px] font-mono px-1.5 py-0.5 bg-accent-purple/20 text-accent-purple rounded">{{ skill.trigger }}</span>
                    <span class="text-[9px] font-mono text-text-tertiary opacity-50">{{ skill.usageCount }}x</span>
                  </div>
                  <p v-if="skill.description" class="text-[10px] text-text-tertiary">{{ skill.description }}</p>
                </div>
                <div class="flex gap-2 opacity-0 group-hover:opacity-100 transition-opacity flex-none">
                  <button class="text-[9px] font-mono text-accent-green hover:text-white transition-colors" @click="copyScript(skill.script)">{{ $t('globalContext.copyAction') }}</button>
                  <button class="text-[9px] font-mono text-accent-blue hover:text-white transition-colors" @click="startSkill(skill)">{{ $t('globalContext.editAction') }}</button>
                  <button class="text-[9px] font-mono text-accent-red hover:text-white transition-colors" @click="deleteSkill(skill.id)">{{ $t('globalContext.deleteAction') }}</button>
                </div>
              </div>
              <pre v-if="skill.script" class="mt-2 text-[10px] font-mono text-text-tertiary bg-black/30 p-2 rounded overflow-x-auto max-h-20">{{ skill.script }}</pre>
            </div>
          </div>
        </div>

        <!-- ── PROMPTS ─────────────────────────────────────────────────────── -->
        <div v-else-if="activeTab === 'prompts'" class="flex flex-col gap-4">
          <div class="flex items-center justify-between">
            <p class="text-[10px] text-text-tertiary font-mono opacity-70">
              Reusable prompts visible to every agent via <code class="bg-bg-sidebar px-1 rounded">global_list_prompts</code>
            </p>
            <button
              class="px-4 py-2 bg-accent-purple text-white text-[10px] font-black uppercase tracking-widest hover:bg-accent-purple/80 transition-colors"
              @click="startPrompt()"
            >{{ $t('globalContext.newPromptButton') }}</button>
          </div>

          <!-- Form -->
          <div v-if="showPromptForm" class="bg-bg-elevated/20 border border-border-primary rounded-lg p-4 flex flex-col gap-3">
            <p class="text-[9px] font-black uppercase tracking-widest text-accent-purple opacity-80">
              {{ editingPrompt ? $t('globalContext.editPrompt') : $t('globalContext.newGlobalPrompt') }}
            </p>
            <div>
              <label class="block text-[9px] text-text-tertiary font-mono mb-1">{{ $t('globalContext.promptTitle') }}</label>
              <input v-model="promptForm.title" class="w-full bg-bg-primary border border-border-primary px-3 py-2 text-[11px] font-mono text-text-primary outline-none focus:border-accent-purple" placeholder="e.g. Code review checklist" />
            </div>
            <div>
              <label class="block text-[9px] text-text-tertiary font-mono mb-1">{{ $t('globalContext.promptContent') }}</label>
              <textarea v-model="promptForm.content" rows="6" class="w-full bg-bg-primary border border-border-primary px-3 py-2 text-[11px] font-mono text-text-primary outline-none focus:border-accent-purple resize-none" placeholder="Prompt text..." />
            </div>
            <div class="flex gap-2">
              <button class="px-4 py-2 bg-accent-purple text-white text-[10px] font-black uppercase tracking-widest hover:bg-accent-purple/80 transition-colors" @click="savePrompt">{{ $t('globalContext.save') }}</button>
              <button class="px-4 py-2 border border-border-primary text-text-tertiary text-[10px] font-black uppercase tracking-widest hover:text-white transition-colors" @click="showPromptForm = false">{{ $t('globalContext.cancel') }}</button>
            </div>
          </div>

          <div v-if="prompts.length === 0 && !showPromptForm" class="py-8 text-center opacity-40">
            <p class="text-[11px] text-text-tertiary font-mono">{{ $t('globalContext.noPrompts') }}</p>
          </div>
          <div v-else class="space-y-2">
            <div
              v-for="prompt in prompts"
              :key="prompt.id"
              class="p-4 bg-bg-elevated/10 border border-border-primary rounded-lg group"
            >
              <div class="flex items-start gap-4">
                <div class="flex-1 min-w-0">
                  <p class="text-[12px] font-black text-text-primary mb-2">{{ prompt.title }}</p>
                  <p class="text-[10px] font-mono text-text-tertiary whitespace-pre-wrap line-clamp-3">{{ prompt.content }}</p>
                </div>
                <div class="flex gap-2 opacity-0 group-hover:opacity-100 transition-opacity flex-none">
                  <button class="text-[9px] font-mono text-accent-green hover:text-white transition-colors" @click="copyPrompt(prompt.content)">{{ $t('globalContext.copyAction') }}</button>
                  <button class="text-[9px] font-mono text-accent-blue hover:text-white transition-colors" @click="startPrompt(prompt)">{{ $t('globalContext.editAction') }}</button>
                  <button class="text-[9px] font-mono text-accent-red hover:text-white transition-colors" @click="deletePrompt(prompt.id)">{{ $t('globalContext.deleteAction') }}</button>
                </div>
              </div>
            </div>
          </div>
        </div>

      </div>
    </div>
      </div>
    </div>
  </Teleport>
</template>
