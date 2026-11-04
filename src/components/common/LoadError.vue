<script setup lang="ts">

defineProps<{ title: string; message: string; action: string; busy: boolean }>();
const emit = defineEmits<{ retry: [] }>();

</script>

<template>
  <div class="error_state" role="alert">
    <span class="error_icon" aria-hidden="true">!</span>
    <div class="error_content">
      <h2>{{ title }}</h2>
      <p class="error_message">{{ message }}</p>
    </div>
    <button
      type="button"
      class="retry_button app_button app_button--primary"
      :disabled="busy"
      @click="emit('retry')"
    >
      {{ action }}
    </button>
  </div>
</template>

<style scoped lang="scss">

.error_state {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: 16px;
  width: min(100%, 720px);
  margin: 24px auto;
  padding: 20px 24px;
  border: 1px solid var(--nas-border);
  border-left: 4px solid var(--nas-error);
  border-radius: 12px;
  background: var(--nas-surface);
  box-shadow: 0 8px 24px #0000000d;
}

.error_icon {
  display: grid;
  place-items: center;
  width: 38px;
  height: 38px;
  border-radius: 50%;
  background: color-mix(in srgb, var(--nas-error) 12%, transparent);
  color: var(--nas-error);
  font-size: 23px;
  font-weight: 700;
}

.error_content {
  min-width: 0;

  h2 {
    margin: 0;
    font-size: 18px;
  }
}

.error_message {
  overflow-wrap: anywhere;
  margin: 4px 0 0;
  color: var(--nas-muted);
}

.retry_button {
  justify-self: end;
}

@media (max-width: 700px) {
  .error_state {
    grid-template-columns: auto minmax(0, 1fr);
    padding: 18px;
  }

  .retry_button {
    grid-column: 2;
    justify-self: start;
  }
}

</style>
