import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { authApi } from '@/api/platform-client';
import type { UserInfo } from '@/api/types';

const TOKEN_KEY = 'ordo_studio_token';

export const useAuthStore = defineStore('auth', () => {
  const token = ref<string | null>(localStorage.getItem(TOKEN_KEY));
  const user = ref<UserInfo | null>(null);

  const isLoggedIn = computed(() => !!token.value);

  function setToken(t: string) {
    token.value = t;
    localStorage.setItem(TOKEN_KEY, t);
  }

  function clearAuth() {
    token.value = null;
    user.value = null;
    localStorage.removeItem(TOKEN_KEY);
  }

  async function login(email: string, password: string) {
    const resp = await authApi.login(email, password);
    setToken(resp.token);
    user.value = resp.user;
    return resp;
  }

  async function register(email: string, password: string, displayName: string) {
    const resp = await authApi.register(email, password, displayName);
    setToken(resp.token);
    user.value = resp.user;
    return resp;
  }

  async function fetchMe() {
    if (!token.value) return;
    try {
      user.value = await authApi.me(token.value);
    } catch (e: any) {
      if (e.status === 401) clearAuth();
    }
  }

  async function updateProfile(patch: { display_name?: string }) {
    if (!token.value) return;
    user.value = await authApi.updateProfile(token.value, patch);
  }

  async function changePassword(currentPassword: string, newPassword: string) {
    if (!token.value) return;
    await authApi.changePassword(token.value, currentPassword, newPassword);
  }

  function logout() {
    clearAuth();
  }

  return {
    token,
    user,
    isLoggedIn,
    login,
    register,
    fetchMe,
    updateProfile,
    changePassword,
    logout,
  };
});
