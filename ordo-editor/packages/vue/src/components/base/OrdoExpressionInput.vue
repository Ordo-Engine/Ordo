<script setup lang="ts">
/**
 * OrdoExpressionInput - Expression input with syntax highlighting and autocomplete
 * 表达式输入组件，支持语法高亮和自动补全
 */
import { computed, ref, watch, onMounted, onUnmounted } from 'vue';

export interface FieldSuggestion {
  /** Field path (e.g., "user.name") */
  path: string;
  /** Display label */
  label: string;
  /** Field type */
  type?: string;
  /** Description */
  description?: string;
}

/** JIT analysis result for the expression */
export interface JITAnalysisResult {
  /** Whether the expression is JIT-compatible */
  jitCompatible: boolean;
  /** Reason for incompatibility */
  reason?: string;
  /** Unsupported features found */
  unsupportedFeatures?: string[];
  /** Supported features used */
  supportedFeatures?: string[];
}

export interface Props {
  /** Expression string */
  modelValue: string;
  /** Placeholder text */
  placeholder?: string;
  /** Available field suggestions */
  suggestions?: FieldSuggestion[];
  /** Whether the input is disabled */
  disabled?: boolean;
  /** Whether to show syntax validation */
  showValidation?: boolean;
  /** Whether to show JIT compatibility indicator */
  showJitIndicator?: boolean;
  /** External JIT analysis result (if provided, skips internal analysis) */
  jitAnalysis?: JITAnalysisResult | null;
  /** Multiline mode */
  multiline?: boolean;
  /** Minimum rows (for multiline) */
  minRows?: number;
  /** Maximum rows (for multiline) */
  maxRows?: number;
}

const props = withDefaults(defineProps<Props>(), {
  placeholder: 'Enter expression...',
  suggestions: () => [],
  disabled: false,
  showValidation: true,
  showJitIndicator: false,
  jitAnalysis: null,
  multiline: false,
  minRows: 1,
  maxRows: 5,
});

const emit = defineEmits<{
  'update:modelValue': [value: string];
  change: [value: string];
  validate: [valid: boolean, error?: string];
  'jit-analysis': [result: JITAnalysisResult];
}>();

// State
const inputRef = ref<HTMLInputElement | HTMLTextAreaElement | null>(null);
const showSuggestions = ref(false);
const selectedSuggestionIndex = ref(0);
const cursorPosition = ref(0);
const validationError = ref<string | null>(null);

// JIT compatibility state
const internalJitAnalysis = ref<JITAnalysisResult | null>(null);
const isAnalyzingJit = ref(false);

// Use external analysis if provided, otherwise use internal
const jitResult = computed(() => props.jitAnalysis ?? internalJitAnalysis.value);

// Simple client-side JIT compatibility check (heuristic-based)
// For accurate analysis, use the WASM analyze_jit_compatibility function
function analyzeJitCompatibility(expr: string): JITAnalysisResult {
  if (!expr.trim()) {
    return { jitCompatible: true };
  }

  const unsupportedFeatures: string[] = [];
  const supportedFeatures: string[] = [];

  // Check for unsupported string operations
  if (expr.includes(' in ') || expr.includes(' contains ') || expr.includes(' not in ')) {
    unsupportedFeatures.push('set_operations');
  }

  // Check for string literals (JIT works best with numeric expressions)
  const stringLiteralMatch = expr.match(/["'][^"']*["']/g);
  if (stringLiteralMatch && stringLiteralMatch.length > 0) {
    unsupportedFeatures.push('string_comparison');
  }

  // Check for array syntax
  if (expr.includes('[') && expr.includes(']')) {
    unsupportedFeatures.push('array_access');
  }

  // Check for supported features
  if (/[<>]=?|==|!=/.test(expr)) {
    supportedFeatures.push('comparison');
  }
  if (/&&|\|\|/.test(expr)) {
    supportedFeatures.push('logical_operations');
  }
  if (/[+\-*/]/.test(expr)) {
    supportedFeatures.push('arithmetic');
  }
  if (/\$\.\w+/.test(expr) || /\b\w+\.\w+\b/.test(expr)) {
    supportedFeatures.push('field_access');
  }

  // Check for supported math functions
  const supportedFuncs = ['abs', 'min', 'max', 'floor', 'ceil', 'round', 'sqrt', 'pow'];
  for (const func of supportedFuncs) {
    if (expr.includes(`${func}(`)) {
      supportedFeatures.push('math_functions');
      break;
    }
  }

  const jitCompatible = unsupportedFeatures.length === 0;

  return {
    jitCompatible,
    reason: jitCompatible ? undefined : `Unsupported: ${unsupportedFeatures.join(', ')}`,
    unsupportedFeatures,
    supportedFeatures,
  };
}

// Filter suggestions based on current word
const currentWord = computed(() => {
  const text = props.modelValue;
  const pos = cursorPosition.value;

  // Find word boundaries
  let start = pos;
  while (start > 0 && /[\w.$]/.test(text[start - 1])) {
    start--;
  }

  return text.slice(start, pos);
});

const filteredSuggestions = computed(() => {
  const word = currentWord.value.toLowerCase();
  if (!word || word.length < 1) return [];

  // Include $ prefix suggestions
  const prefix = word.startsWith('$') ? word.slice(1) : word;

  return props.suggestions
    .filter((s) => {
      const searchPath = s.path.toLowerCase();
      const searchLabel = s.label.toLowerCase();
      return searchPath.includes(prefix) || searchLabel.includes(prefix);
    })
    .slice(0, 10);
});

// Simple expression validation
function validateExpression(expr: string): { valid: boolean; error?: string } {
  if (!expr.trim()) {
    return { valid: true };
  }

  // Check for balanced parentheses
  let parenCount = 0;
  let bracketCount = 0;
  let braceCount = 0;

  for (const char of expr) {
    if (char === '(') parenCount++;
    else if (char === ')') parenCount--;
    else if (char === '[') bracketCount++;
    else if (char === ']') bracketCount--;
    else if (char === '{') braceCount++;
    else if (char === '}') braceCount--;

    if (parenCount < 0 || bracketCount < 0 || braceCount < 0) {
      return { valid: false, error: 'Unbalanced brackets' };
    }
  }

  if (parenCount !== 0) return { valid: false, error: 'Unbalanced parentheses' };
  if (bracketCount !== 0) return { valid: false, error: 'Unbalanced square brackets' };
  if (braceCount !== 0) return { valid: false, error: 'Unbalanced curly braces' };

  // Check for unclosed strings
  let inString = false;
  let stringChar = '';

  for (let i = 0; i < expr.length; i++) {
    const char = expr[i];
    const prevChar = i > 0 ? expr[i - 1] : '';

    if (!inString && (char === '"' || char === "'")) {
      inString = true;
      stringChar = char;
    } else if (inString && char === stringChar && prevChar !== '\\') {
      inString = false;
    }
  }

  if (inString) {
    return { valid: false, error: 'Unclosed string' };
  }

  return { valid: true };
}

// Watch for changes
watch(
  () => props.modelValue,
  (newVal) => {
    if (props.showValidation) {
      const result = validateExpression(newVal);
      validationError.value = result.error || null;
      emit('validate', result.valid, result.error);
    }

    // Analyze JIT compatibility if enabled and no external analysis provided
    if (props.showJitIndicator && !props.jitAnalysis) {
      const jitResult = analyzeJitCompatibility(newVal);
      internalJitAnalysis.value = jitResult;
      emit('jit-analysis', jitResult);
    }
  },
  { immediate: true }
);

// Handle input
function handleInput(event: Event) {
  const target = event.target as HTMLInputElement | HTMLTextAreaElement;
  cursorPosition.value = target.selectionStart || 0;
  emit('update:modelValue', target.value);

  // Show suggestions if typing a variable
  if (currentWord.value.startsWith('$') || currentWord.value.length > 0) {
    showSuggestions.value = filteredSuggestions.value.length > 0;
    selectedSuggestionIndex.value = 0;
  } else {
    showSuggestions.value = false;
  }
}

function handleBlur() {
  // Delay hiding to allow click on suggestion
  setTimeout(() => {
    showSuggestions.value = false;
    emit('change', props.modelValue);
  }, 150);
}

function handleKeyDown(event: KeyboardEvent) {
  if (!showSuggestions.value) return;

  switch (event.key) {
    case 'ArrowDown':
      event.preventDefault();
      selectedSuggestionIndex.value = Math.min(
        selectedSuggestionIndex.value + 1,
        filteredSuggestions.value.length - 1
      );
      break;

    case 'ArrowUp':
      event.preventDefault();
      selectedSuggestionIndex.value = Math.max(selectedSuggestionIndex.value - 1, 0);
      break;

    case 'Enter':
      if (filteredSuggestions.value.length > 0) {
        event.preventDefault();
        selectSuggestion(filteredSuggestions.value[selectedSuggestionIndex.value]);
      }
      break;

    case 'Escape':
      showSuggestions.value = false;
      break;
  }
}

function selectSuggestion(suggestion: FieldSuggestion) {
  const text = props.modelValue;
  const pos = cursorPosition.value;

  // Find word boundaries
  let start = pos;
  while (start > 0 && /[\w.$]/.test(text[start - 1])) {
    start--;
  }

  // Replace current word with suggestion
  const before = text.slice(0, start);
  const after = text.slice(pos);
  const newValue = `${before}$.${suggestion.path}${after}`;

  emit('update:modelValue', newValue);
  showSuggestions.value = false;

  // Focus and move cursor
  setTimeout(() => {
    if (inputRef.value) {
      inputRef.value.focus();
      const newPos = start + suggestion.path.length + 2; // +2 for "$."
      inputRef.value.setSelectionRange(newPos, newPos);
    }
  }, 0);
}

// Track cursor position
function updateCursorPosition() {
  if (inputRef.value) {
    cursorPosition.value = inputRef.value.selectionStart || 0;
  }
}

// Keyboard shortcut: Ctrl+Space to show suggestions
function handleGlobalKeyDown(event: KeyboardEvent) {
  if (event.ctrlKey && event.key === ' ' && document.activeElement === inputRef.value) {
    event.preventDefault();
    showSuggestions.value = props.suggestions.length > 0;
  }
}

onMounted(() => {
  document.addEventListener('keydown', handleGlobalKeyDown);
});

onUnmounted(() => {
  document.removeEventListener('keydown', handleGlobalKeyDown);
});
</script>

<template>
  <div class="ordo-expression-input" :class="{ disabled, invalid: !!validationError }">
    <!-- Input field -->
    <div class="ordo-expression-input__wrapper">
      <component
        :is="multiline ? 'textarea' : 'input'"
        ref="inputRef"
        :value="modelValue"
        :placeholder="placeholder"
        :disabled="disabled"
        :rows="multiline ? minRows : undefined"
        class="ordo-expression-input__field"
        :class="{ 'is-multiline': multiline }"
        @input="handleInput"
        @blur="handleBlur"
        @keydown="handleKeyDown"
        @click="updateCursorPosition"
        @keyup="updateCursorPosition"
      />

      <!-- Indicators container -->
      <div v-if="(showValidation || showJitIndicator) && modelValue" class="ordo-expression-input__indicators">
        <!-- JIT compatibility indicator -->
        <span
          v-if="showJitIndicator && jitResult"
          class="ordo-expression-input__status jit"
          :class="{ compatible: jitResult.jitCompatible, incompatible: !jitResult.jitCompatible }"
          :title="jitResult.jitCompatible 
            ? 'JIT Compatible - Can be compiled for ~20x faster execution' 
            : `Not JIT Compatible: ${jitResult.reason || 'Unknown reason'}`"
        >
          <!-- Lightning bolt icon for JIT -->
          <svg
            width="12"
            height="12"
            viewBox="0 0 24 24"
            fill="currentColor"
            stroke="none"
          >
            <path d="M13 2L3 14h8l-1 8 10-12h-8l1-8z" />
          </svg>
        </span>

        <!-- Validation indicator -->
        <span
          v-if="showValidation && validationError"
          class="ordo-expression-input__status error"
          :title="validationError"
        >
          <svg
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
          >
            <circle cx="12" cy="12" r="10"></circle>
            <line x1="12" y1="8" x2="12" y2="12"></line>
            <line x1="12" y1="16" x2="12.01" y2="16"></line>
          </svg>
        </span>
        <span v-else-if="showValidation" class="ordo-expression-input__status success">
          <svg
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
          >
            <polyline points="20 6 9 17 4 12"></polyline>
          </svg>
        </span>
      </div>
    </div>

    <!-- Suggestions dropdown -->
    <Transition name="ordo-fade">
      <div v-if="showSuggestions" class="ordo-expression-input__suggestions">
        <div
          v-for="(suggestion, index) in filteredSuggestions"
          :key="suggestion.path"
          class="ordo-expression-input__suggestion"
          :class="{ selected: index === selectedSuggestionIndex }"
          @mousedown.prevent="selectSuggestion(suggestion)"
        >
          <div class="ordo-expression-input__suggestion-main">
            <span class="ordo-expression-input__suggestion-path">$.{{ suggestion.path }}</span>
            <span v-if="suggestion.description" class="ordo-expression-input__suggestion-desc">
              {{ suggestion.description }}
            </span>
          </div>
          <span v-if="suggestion.type" class="ordo-expression-input__suggestion-type">
            {{ suggestion.type }}
          </span>
        </div>
      </div>
    </Transition>

    <!-- Validation error message -->
    <div v-if="validationError" class="ordo-expression-input__error-message">
      {{ validationError }}
    </div>
  </div>
</template>

<style scoped>
.ordo-expression-input {
  position: relative;
  width: 100%;
}

.ordo-expression-input__wrapper {
  position: relative;
  display: flex;
  align-items: center;
}

.ordo-expression-input__field {
  width: 100%;
  height: 32px;
  padding: 0 28px 0 var(--ordo-space-sm); /* Right padding for indicator */
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-md);
  font-size: var(--ordo-font-size-sm);
  font-family: var(--ordo-font-mono);
  background: var(--ordo-bg-input);
  color: var(--ordo-text-primary);
  transition: var(--ordo-transition-base);
}

.ordo-expression-input__field.is-multiline {
  height: auto;
  min-height: 32px;
  padding-top: 6px;
  padding-bottom: 6px;
  line-height: 1.5;
  resize: vertical;
}

.ordo-expression-input__field:focus {
  outline: none;
  border-color: var(--ordo-primary-500);
  box-shadow: var(--ordo-focus-ring);
}

.ordo-expression-input__field:hover:not(:disabled):not(:focus) {
  border-color: var(--ordo-border-hover);
}

.ordo-expression-input.invalid .ordo-expression-input__field {
  border-color: var(--ordo-error);
  background-color: var(--ordo-error-bg);
}

.ordo-expression-input.invalid .ordo-expression-input__field:focus {
  box-shadow: 0 0 0 3px var(--ordo-error-alpha);
}

.ordo-expression-input__field:disabled {
  background: var(--ordo-bg-disabled);
  color: var(--ordo-text-tertiary);
  cursor: not-allowed;
}

.ordo-expression-input__indicators {
  position: absolute;
  right: 8px;
  top: 50%;
  transform: translateY(-50%);
  display: flex;
  align-items: center;
  gap: 4px;
  pointer-events: none;
}

.ordo-expression-input__field.is-multiline + .ordo-expression-input__indicators {
  top: 10px;
  transform: none;
}

.ordo-expression-input__status {
  display: flex;
  align-items: center;
  justify-content: center;
}

.ordo-expression-input__status.error {
  color: var(--ordo-error);
}

.ordo-expression-input__status.success {
  color: var(--ordo-success);
}

/* JIT Compatibility Indicator */
.ordo-expression-input__status.jit {
  padding: 2px;
  border-radius: 3px;
  transition: all 0.15s ease;
}

.ordo-expression-input__status.jit.compatible {
  color: #f59e0b; /* Amber/gold for lightning */
  background: rgba(245, 158, 11, 0.15);
}

.ordo-expression-input__status.jit.incompatible {
  color: var(--ordo-text-tertiary);
  opacity: 0.4;
}

/* Tooltip styling would be added via a tooltip component */
.ordo-expression-input__status.jit:hover {
  pointer-events: auto;
  cursor: help;
}

/* Suggestions Dropdown */
.ordo-expression-input__suggestions {
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  right: 0;
  background: var(--ordo-bg-popup);
  border: 1px solid var(--ordo-border-color);
  border-radius: var(--ordo-radius-md);
  box-shadow: var(--ordo-shadow-lg);
  z-index: var(--ordo-z-dropdown);
  max-height: 240px;
  overflow-y: auto;
}

.ordo-expression-input__suggestion {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 10px;
  cursor: pointer;
  transition: background-color 0.1s;
  border-bottom: 1px solid var(--ordo-gray-100);
}

[data-ordo-theme='dark'] .ordo-expression-input__suggestion {
  border-bottom-color: var(--ordo-gray-800);
}

.ordo-expression-input__suggestion:last-child {
  border-bottom: none;
}

.ordo-expression-input__suggestion:hover,
.ordo-expression-input__suggestion.selected {
  background: var(--ordo-primary-50);
}

[data-ordo-theme='dark'] .ordo-expression-input__suggestion:hover,
[data-ordo-theme='dark'] .ordo-expression-input__suggestion.selected {
  background: var(--ordo-gray-800);
}

.ordo-expression-input__suggestion-main {
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.ordo-expression-input__suggestion-path {
  font-family: var(--ordo-font-mono);
  font-size: 13px;
  font-weight: 500;
  color: var(--ordo-text-primary);
}

.ordo-expression-input__suggestion-desc {
  font-size: 11px;
  color: var(--ordo-text-tertiary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.ordo-expression-input__suggestion-type {
  font-size: 10px;
  padding: 2px 6px;
  background: var(--ordo-bg-tertiary);
  color: var(--ordo-text-secondary);
  border-radius: var(--ordo-radius-xs);
  margin-left: 8px;
  font-family: var(--ordo-font-mono);
}

.ordo-expression-input__error-message {
  margin-top: 4px;
  font-size: 11px;
  color: var(--ordo-error);
  padding-left: 2px;
}

/* Transitions */
.ordo-fade-enter-active,
.ordo-fade-leave-active {
  transition:
    opacity 0.15s ease,
    transform 0.15s ease;
}

.ordo-fade-enter-from,
.ordo-fade-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}
</style>
