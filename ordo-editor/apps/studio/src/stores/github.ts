import { defineStore } from 'pinia';
import { ref } from 'vue';
import { githubApi } from '@/api/platform-client';
import { useAuthStore } from './auth';
import type { GitHubStatus } from '@/api/types';

export const useGithubStore = defineStore('github', () => {
  const status = ref<GitHubStatus>({ connected: false });
  const loading = ref(false);

  async function fetchStatus() {
    const auth = useAuthStore();
    if (!auth.token) return;
    loading.value = true;
    try {
      status.value = await githubApi.getStatus(auth.token);
    } catch {
      status.value = { connected: false };
    } finally {
      loading.value = false;
    }
  }

  async function getConnectUrl(): Promise<string> {
    const auth = useAuthStore();
    const { url } = await githubApi.getConnectUrl(auth.token!);
    return url;
  }

  async function connect(): Promise<void> {
    const url = await getConnectUrl();
    let callbackOrigin = window.location.origin;
    try {
      const authUrl = new URL(url);
      const redirectUri = authUrl.searchParams.get('redirect_uri');
      if (redirectUri) {
        callbackOrigin = new URL(redirectUri).origin;
      }
    } catch {
      callbackOrigin = window.location.origin;
    }

    return new Promise((resolve, reject) => {
      const popup = window.open(url, 'github-oauth', 'width=600,height=700,scrollbars=yes');
      if (!popup) {
        reject(new Error('Popup blocked — please allow popups for this site'));
        return;
      }

      const onMessage = async (event: MessageEvent) => {
        if (event.origin !== window.location.origin && event.origin !== callbackOrigin) return;
        if (event.data?.type === 'github_connected') {
          window.removeEventListener('message', onMessage);
          await fetchStatus();
          resolve();
        } else if (event.data?.type === 'github_error') {
          window.removeEventListener('message', onMessage);
          reject(new Error(event.data.error || 'GitHub connection failed'));
        }
      };
      window.addEventListener('message', onMessage);

      // Fallback: if popup closes without message, re-check status
      const timer = setInterval(() => {
        if (popup.closed) {
          clearInterval(timer);
          window.removeEventListener('message', onMessage);
          fetchStatus().then(resolve).catch(reject);
        }
      }, 500);
    });
  }

  async function disconnect() {
    const auth = useAuthStore();
    if (!auth.token) return;
    await githubApi.disconnect(auth.token);
    status.value = { connected: false };
  }

  return { status, loading, fetchStatus, connect, disconnect };
});
