<template>
  <div class="studio-dialog-actions">
    <slot />
  </div>
</template>

<style scoped>
.studio-dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 20px;
  padding-top: 16px;
  border-top: 1px solid var(--ordo-border-color);
}
</style>
