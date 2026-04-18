<template>
  <div class="dialog-overlay" @click.self="$emit('close')">
    <div class="dialog">
      <h3 class="dialog-title">{{ $t('publish.title') }}</h3>

      <div class="field">
        <label>{{ $t('publish.environment') }}</label>
        <select v-model="selectedEnvId" class="input">
          <option v-if="environments.length === 0" value="" disabled>{{ $t('publish.noEnvs') }}</option>
          <option v-for="env in environments" :key="env.id" :value="env.id">
            {{ env.name }}{{ env.is_default ? ' ★' : '' }}
          </option>
        </select>
      </div>

      <div class="field">
        <label>{{ $t('publish.releaseNote') }}</label>
        <textarea
          v-model="releaseNote"
          class="input textarea"
          :placeholder="$t('publish.releaseNotePlaceholder')"
          rows="3"
        />
      </div>

      <div v-if="error" class="error-msg">{{ error }}</div>

      <div class="dialog-actions">
        <button class="btn-secondary" @click="$emit('close')">{{ $t('common.cancel') }}</button>
        <button
          class="btn-primary"
          :disabled="!selectedEnvId || publishing"
          @click="handlePublish"
        >
          {{ publishing ? $t('publish.publishing') : $t('publish.btn') }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { rulesetDraftApi } from '@/api/platform-client'
import type { ProjectEnvironment, RulesetDeployment } from '@/api/types'
import { useAuthStore } from '@/stores/auth'

const props = defineProps<{
  orgId: string
  projectId: string
  rulesetName: string
  environments: ProjectEnvironment[]
}>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'published', deployment: RulesetDeployment): void
}>()

const { t } = useI18n()
const auth = useAuthStore()

const selectedEnvId = ref(props.environments.find((e) => e.is_default)?.id ?? '')
const releaseNote = ref('')
const publishing = ref(false)
const error = ref<string | null>(null)

async function handlePublish() {
  if (!selectedEnvId.value || !auth.token) return
  publishing.value = true
  error.value = null
  try {
    const deployment = await rulesetDraftApi.publish(
      auth.token,
      props.orgId,
      props.projectId,
      props.rulesetName,
      { environment_id: selectedEnvId.value, release_note: releaseNote.value || undefined },
    )
    emit('published', deployment)
  } catch (e: any) {
    error.value = e.message || t('publish.failed')
  } finally {
    publishing.value = false
  }
}
</script>

<style scoped>
.dialog-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}
.dialog {
  background: var(--surface-color, #1e1e2e);
  border: 1px solid var(--border-color, #313244);
  border-radius: 8px;
  padding: 24px;
  width: 440px;
  max-width: 90vw;
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.dialog-title {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
}
.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.field label {
  font-size: 12px;
  color: var(--text-secondary, #a6adc8);
}
.input {
  background: var(--input-bg, #313244);
  border: 1px solid var(--border-color, #45475a);
  border-radius: 4px;
  padding: 8px 10px;
  color: inherit;
  font-size: 13px;
  outline: none;
}
.input:focus {
  border-color: var(--accent-color, #cba6f7);
}
.textarea {
  resize: vertical;
  font-family: inherit;
}
.error-msg {
  color: var(--error-color, #f38ba8);
  font-size: 12px;
}
.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
.btn-primary {
  padding: 8px 16px;
  border-radius: 4px;
  border: none;
  cursor: pointer;
  background: var(--accent-color, #cba6f7);
  color: #1e1e2e;
  font-weight: 600;
  font-size: 13px;
}
.btn-primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.btn-secondary {
  padding: 8px 16px;
  border-radius: 4px;
  border: 1px solid var(--border-color, #45475a);
  cursor: pointer;
  background: transparent;
  color: inherit;
  font-size: 13px;
}
</style>
