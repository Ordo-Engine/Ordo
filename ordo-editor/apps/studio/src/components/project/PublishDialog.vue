<template>
  <t-dialog
    :visible="true"
    :header="$t('publish.title')"
    width="520px"
    :footer="false"
    destroy-on-close
    @close="$emit('close')"
    @update:visible="(value: boolean) => { if (!value) $emit('close') }"
  >
    <t-form label-align="top" :colon="false" class="publish-form">
      <t-form-item :label="$t('publish.environment')" required>
        <t-select v-model="selectedEnvId" :placeholder="$t('publish.noEnvs')" :disabled="publishing">
          <t-option
            v-for="env in environments"
            :key="env.id"
            :label="`${env.name}${env.is_default ? ' ★' : ''}`"
            :value="env.id"
          />
        </t-select>
      </t-form-item>

      <t-form-item :label="$t('publish.releaseNote')">
        <t-textarea
          v-model="releaseNote"
          :placeholder="$t('publish.releaseNotePlaceholder')"
          :autosize="{ minRows: 3, maxRows: 5 }"
          :disabled="publishing"
        />
      </t-form-item>
    </t-form>

    <t-alert v-if="error" theme="error" :message="error" class="publish-alert" />

    <StudioDialogActions>
      <t-button variant="outline" @click="$emit('close')">{{ $t('common.cancel') }}</t-button>
      <t-button
        theme="primary"
        :disabled="!selectedEnvId || publishing || environments.length === 0"
        :loading="publishing"
        @click="handlePublish"
      >
        {{ publishing ? $t('publish.publishing') : $t('publish.btn') }}
      </t-button>
    </StudioDialogActions>
  </t-dialog>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { rulesetDraftApi } from '@/api/platform-client'
import type { ProjectEnvironment, RulesetDeployment } from '@/api/types'
import { useAuthStore } from '@/stores/auth'
import { StudioDialogActions } from '@/components/ui'

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
.publish-form {
  padding-top: 4px;
}

.publish-alert {
  margin-top: 8px;
}

</style>
