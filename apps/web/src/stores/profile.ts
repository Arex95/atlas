import { ref, computed } from 'vue';
import { defineStore } from 'pinia';
import { api } from '@/api/client';

export interface UserProfile {
  name: string;
  title: string;
  email: string;
  github: string;
  website: string;
  avatarColor: string;
}

const defaults: UserProfile = {
  name: 'Developer',
  title: 'Atlas User',
  email: '',
  github: '',
  website: '',
  avatarColor: '#3b82f6',
};

export const useProfileStore = defineStore('profile', () => {
  const profile = ref<UserProfile>({ ...defaults });
  const loaded = ref(false);

  const initials = computed(() => {
    const parts = profile.value.name.trim().split(/\s+/);
    if (parts.length >= 2) return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase();
    return profile.value.name.slice(0, 2).toUpperCase() || 'U';
  });

  async function fetch() {
    try {
      const data = await api.get<UserProfile>('/api/profile');
      profile.value = data;
    } catch {
      // server not ready yet — use defaults silently
    } finally {
      loaded.value = true;
    }
  }

  async function update(patch: Partial<UserProfile>) {
    const merged = { ...profile.value, ...patch };
    const saved = await api.put<UserProfile>('/api/profile', merged);
    profile.value = saved;
  }

  return { profile, initials, loaded, fetch, update };
});
