<script setup lang="ts">
import { ref } from 'vue';
import { trackEvent, AnalyticsEvents } from '../utils/analytics';

const emit = defineEmits<{
  (e: 'start-tour'): void;
  (e: 'skip'): void;
}>();

const isVisible = ref(true);

function startTour() {
  isVisible.value = false;
  emit('start-tour');
}

function skip() {
  isVisible.value = false;
  trackEvent(AnalyticsEvents.TOUR_SKIPPED);
  emit('skip');
}
</script>

<template>
  <Teleport to="body">
    <Transition name="fade">
      <div v-if="isVisible" class="welcome-overlay" @click.self="skip">
        <div class="welcome-modal">
          <div class="welcome-icon">
            <!-- Logo / Brand Icon -->
            <svg
              width="64"
              height="64"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
            >
              <path d="M12 2L2 7l10 5 10-5-10-5z" />
              <path d="M2 17l10 5 10-5" />
              <path d="M2 12l10 5 10-5" />
            </svg>
          </div>

          <h1>Welcome to Ordo Playground</h1>

          <p class="welcome-desc">
            Ordo is a high-performance rule engine with a visual editor.<br />
            Design complex business rules with ease.
          </p>

          <div class="features">
            <div class="feature">
              <span class="feature-icon">
                <!-- Palette / Design icon -->
                <svg
                  width="20"
                  height="20"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                >
                  <circle cx="12" cy="12" r="10" />
                  <circle cx="12" cy="12" r="3" />
                  <path d="M12 2v4M12 18v4M2 12h4M18 12h4" />
                </svg>
              </span>
              <span>Visual Flow Editor</span>
            </div>
            <div class="feature">
              <span class="feature-icon">
                <!-- Zap / Lightning icon -->
                <svg
                  width="20"
                  height="20"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                >
                  <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" />
                </svg>
              </span>
              <span>WASM-powered Execution</span>
            </div>
            <div class="feature">
              <span class="feature-icon">
                <!-- Search / Trace icon -->
                <svg
                  width="20"
                  height="20"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                >
                  <circle cx="11" cy="11" r="8" />
                  <path d="M21 21l-4.35-4.35" />
                </svg>
              </span>
              <span>Step-by-step Tracing</span>
            </div>
          </div>

          <div class="actions">
            <button class="btn-primary" @click="startTour">
              Take a Quick Tour
              <svg
                width="16"
                height="16"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
              >
                <path d="M5 12h14M12 5l7 7-7 7" />
              </svg>
            </button>
            <button class="btn-secondary" @click="skip">Skip, I'll explore myself</button>
          </div>

          <p class="hint">
            <svg
              width="12"
              height="12"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              style="vertical-align: middle; margin-right: 4px"
            >
              <circle cx="12" cy="12" r="10" />
              <path d="M12 16v-4M12 8h.01" />
            </svg>
            You can restart the tour anytime from the help button
          </p>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.welcome-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.8);
  backdrop-filter: blur(4px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 10000;
}

.welcome-modal {
  background: var(--ordo-bg-panel, #1e1e1e);
  border: 1px solid var(--ordo-border-color, #333);
  border-radius: 16px;
  padding: 40px 48px;
  max-width: 480px;
  text-align: center;
  box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.5);
}

.welcome-icon {
  color: var(--ordo-accent, #007acc);
  margin-bottom: 16px;
}

h1 {
  font-size: 24px;
  font-weight: 600;
  color: var(--ordo-text-primary, #fff);
  margin: 0 0 12px;
}

.welcome-desc {
  color: var(--ordo-text-secondary, #999);
  font-size: 14px;
  line-height: 1.6;
  margin: 0 0 24px;
}

.features {
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin-bottom: 32px;
}

.feature {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 16px;
  background: var(--ordo-bg-secondary, #252525);
  border-radius: 8px;
  font-size: 14px;
  color: var(--ordo-text-primary, #fff);
}

.feature-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  background: var(--ordo-accent-bg, rgba(0, 122, 204, 0.15));
  border-radius: 6px;
  color: var(--ordo-accent, #007acc);
  flex-shrink: 0;
}

.actions {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.btn-primary {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 14px 24px;
  background: var(--ordo-accent, #007acc);
  color: #fff;
  border: none;
  border-radius: 8px;
  font-size: 15px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
}

.btn-primary:hover {
  background: #0098ff;
  transform: translateY(-1px);
}

.btn-secondary {
  padding: 12px 24px;
  background: transparent;
  color: var(--ordo-text-secondary, #999);
  border: 1px solid var(--ordo-border-color, #333);
  border-radius: 8px;
  font-size: 14px;
  cursor: pointer;
  transition: all 0.2s;
}

.btn-secondary:hover {
  background: var(--ordo-bg-secondary, #252525);
  color: var(--ordo-text-primary, #fff);
}

.hint {
  margin: 16px 0 0;
  font-size: 12px;
  color: var(--ordo-text-tertiary, #666);
}

/* Transition */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.3s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

.fade-enter-active .welcome-modal,
.fade-leave-active .welcome-modal {
  transition:
    transform 0.3s ease,
    opacity 0.3s ease;
}

.fade-enter-from .welcome-modal,
.fade-leave-to .welcome-modal {
  transform: scale(0.95);
  opacity: 0;
}
</style>
