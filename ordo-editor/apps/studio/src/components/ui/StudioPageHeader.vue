<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    title: string
    subtitle?: string
  }>(),
  {
    subtitle: '',
  },
)
</script>

<template>
  <div class="studio-page-header">
    <div class="studio-page-header__main">
      <h2 class="studio-page-header__title">{{ props.title }}</h2>
      <p v-if="props.subtitle" class="studio-page-header__subtitle">{{ props.subtitle }}</p>
    </div>
    <div v-if="$slots.actions" class="studio-page-header__actions">
      <slot name="actions" />
    </div>
  </div>
</template>

<style scoped>
.studio-page-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 24px;
}

.studio-page-header__main {
  min-width: 0;
}

.studio-page-header__title {
  margin: 0 0 4px;
  font-size: 20px;
  font-weight: 600;
  color: var(--ordo-text-primary);
}

.studio-page-header__subtitle {
  margin: 0;
  font-size: 13px;
  color: var(--ordo-text-secondary);
}

.studio-page-header__actions {
  display: flex;
  align-items: center;
  gap: 10px;
}

@media (max-width: 900px) {
  .studio-page-header {
    flex-direction: column;
    align-items: stretch;
  }

  .studio-page-header__actions {
    justify-content: flex-start;
  }
}
</style>
