<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { usePersistentNotificationStore } from '@/stores/persistentNotifications';
import { useOrgStore } from '@/stores/org';

const { t } = useI18n();
const router = useRouter();
const orgStore = useOrgStore();
const store = usePersistentNotificationStore();

const orgId = computed(() => orgStore.currentOrg?.id ?? '');
const unreadOnly = ref(false);
const loading = ref(false);

onMounted(async () => {
  if (!orgId.value) return;
  loading.value = true;
  try {
    await store.fetchNotifications(orgId.value, false);
  } finally {
    loading.value = false;
  }
});

const filtered = computed(() =>
  unreadOnly.value ? store.notifications.filter((n) => !n.read_at) : store.notifications
);

function notifIcon(type: string) {
  if (type === 'release_approved') return 'check-circle';
  if (type === 'release_rejected') return 'close-circle';
  if (type === 'release_review_requested') return 'notification';
  return 'info-circle';
}

function notifIconClass(type: string) {
  if (type === 'release_approved') return 'notif-icon--success';
  if (type === 'release_rejected') return 'notif-icon--error';
  return 'notif-icon--info';
}

function notifTitle(type: string, payload: Record<string, unknown>) {
  const releaseTitle = (payload.title as string) ?? '';
  if (type === 'release_review_requested')
    return t('notifications.types.releaseReviewRequested', { title: releaseTitle });
  if (type === 'release_approved')
    return t('notifications.types.releaseApproved', { title: releaseTitle });
  if (type === 'release_rejected')
    return t('notifications.types.releaseRejected', { title: releaseTitle });
  return type;
}

function formatTime(isoDate: string) {
  const diff = Date.now() - new Date(isoDate).getTime();
  if (diff < 60_000) return t('shell.justNow');
  if (diff < 3_600_000) return t('shell.minutesAgo', { n: Math.floor(diff / 60_000) });
  if (diff < 86_400_000) return t('shell.hoursAgo', { n: Math.floor(diff / 3_600_000) });
  return new Date(isoDate).toLocaleDateString();
}

async function handleMarkAllRead() {
  await store.markAllRead(orgId.value);
}

async function handleMarkRead(notifId: string) {
  await store.markRead(orgId.value, notifId);
}

function navigateToRelease(n: { ref_id?: string; payload: Record<string, unknown> }) {
  const projectId = n.payload.project_id as string | undefined;
  if (n.ref_id && projectId && orgId.value) {
    router.push(`/orgs/${orgId.value}/projects/${projectId}/releases`);
  }
}
</script>

<template>
  <div class="notifications-page">
    <div class="notifications-header">
      <h2>{{ t('notifications.title') }}</h2>
      <div class="notifications-header__actions">
        <t-checkbox v-model="unreadOnly">{{ t('notifications.unreadOnly') }}</t-checkbox>
        <t-button
          v-if="store.unreadCount > 0"
          variant="text"
          size="small"
          @click="handleMarkAllRead"
        >
          {{ t('notifications.markAllRead') }}
        </t-button>
      </div>
    </div>

    <div v-if="loading" class="notifications-loading">
      <t-loading />
    </div>

    <div v-else-if="filtered.length === 0" class="notifications-empty">
      <t-icon name="notification" size="48px" style="opacity: 0.2" />
      <p>{{ t('notifications.empty') }}</p>
    </div>

    <div v-else class="notifications-list">
      <div
        v-for="n in filtered"
        :key="n.id"
        class="notif-row"
        :class="{ 'notif-row--unread': !n.read_at }"
        @click="navigateToRelease(n)"
      >
        <span class="notif-row__icon" :class="notifIconClass(n.type)">
          <t-icon :name="notifIcon(n.type)" size="16px" />
        </span>
        <div class="notif-row__body">
          <div class="notif-row__title">{{ notifTitle(n.type, n.payload) }}</div>
          <div class="notif-row__time">{{ formatTime(n.created_at) }}</div>
        </div>
        <button v-if="!n.read_at" class="notif-row__mark-read" @click.stop="handleMarkRead(n.id)">
          <t-icon name="check" size="14px" />
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.notifications-page {
  max-width: 720px;
  margin: 0 auto;
  padding: 32px 24px;
}

.notifications-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 24px;
}

.notifications-header h2 {
  font-size: 20px;
  font-weight: 600;
  margin: 0;
}

.notifications-header__actions {
  display: flex;
  align-items: center;
  gap: 12px;
}

.notifications-loading,
.notifications-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 60px 0;
  color: var(--ordo-text-tertiary);
}

.notifications-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.notif-row {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 12px 16px;
  border-radius: 8px;
  cursor: pointer;
  transition: background 0.15s;
  position: relative;
}

.notif-row:hover {
  background: var(--ordo-bg-hover);
}

.notif-row--unread {
  background: color-mix(in srgb, var(--ordo-accent) 5%, transparent);
}

.notif-row--unread::before {
  content: '';
  position: absolute;
  left: 4px;
  top: 50%;
  transform: translateY(-50%);
  width: 4px;
  height: 4px;
  border-radius: 50%;
  background: var(--ordo-accent);
}

.notif-row__icon {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.notif-icon--success {
  background: color-mix(in srgb, #52c41a 15%, transparent);
  color: #52c41a;
}

.notif-icon--error {
  background: color-mix(in srgb, #ff4d4f 15%, transparent);
  color: #ff4d4f;
}

.notif-icon--info {
  background: color-mix(in srgb, var(--ordo-accent) 15%, transparent);
  color: var(--ordo-accent);
}

.notif-row__body {
  flex: 1;
  min-width: 0;
}

.notif-row__title {
  font-size: 14px;
  font-weight: 500;
  line-height: 1.4;
}

.notif-row__time {
  font-size: 12px;
  color: var(--ordo-text-tertiary);
  margin-top: 2px;
}

.notif-row__mark-read {
  opacity: 0;
  transition: opacity 0.15s;
  padding: 4px;
  border-radius: 4px;
  color: var(--ordo-text-tertiary);
  cursor: pointer;
  border: none;
  background: none;
}

.notif-row:hover .notif-row__mark-read {
  opacity: 1;
}
</style>
