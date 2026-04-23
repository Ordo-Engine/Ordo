<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from 'vue';
import { useRouter } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { useProjectStore } from '@/stores/project';
import { useCatalogStore } from '@/stores/catalog';
import { useOrgStore } from '@/stores/org';

const { t } = useI18n();
const router = useRouter();
const projectStore = useProjectStore();
const catalogStore = useCatalogStore();
const orgStore = useOrgStore();

const visible = ref(false);
const query = ref('');
const selectedIdx = ref(0);
const inputRef = ref<HTMLInputElement | null>(null);

interface ResultItem {
  id: string;
  label: string;
  desc?: string;
  group: string;
  action: () => void;
}

const results = computed<ResultItem[]>(() => {
  const q = query.value.toLowerCase().trim();
  const items: ResultItem[] = [];

  // Projects
  for (const p of projectStore.projects) {
    if (!q || p.name.toLowerCase().includes(q)) {
      items.push({
        id: `project-${p.id}`,
        label: p.name,
        desc: p.description ?? undefined,
        group: t('search.groupProjects'),
        action: () => {
          const oid = orgStore.currentOrg?.id;
          if (oid) router.push(`/orgs/${oid}/projects/${p.id}/editor`);
        },
      });
    }
  }

  // Rulesets
  for (const r of projectStore.rulesets) {
    if (!q || r.name.toLowerCase().includes(q)) {
      const pid = projectStore.currentProject?.id;
      items.push({
        id: `ruleset-${r.name}`,
        label: r.name,
        desc: projectStore.currentProject?.name,
        group: t('search.groupRulesets'),
        action: () => {
          const oid = orgStore.currentOrg?.id;
          if (oid && pid) router.push(`/orgs/${oid}/projects/${pid}/editor/${r.name}`);
        },
      });
    }
  }

  // Facts
  for (const f of catalogStore.facts) {
    if (!q || f.name.toLowerCase().includes(q)) {
      items.push({
        id: `fact-${f.name}`,
        label: f.name,
        desc: f.description ?? undefined,
        group: t('search.groupFacts'),
        action: () => {
          const pid = projectStore.currentProject?.id;
          const oid = orgStore.currentOrg?.id;
          if (oid && pid) router.push(`/orgs/${oid}/projects/${pid}/facts`);
        },
      });
    }
  }

  // Concepts
  for (const c of catalogStore.concepts) {
    if (!q || c.name.toLowerCase().includes(q)) {
      items.push({
        id: `concept-${c.name}`,
        label: c.name,
        desc: c.description ?? undefined,
        group: t('search.groupConcepts'),
        action: () => {
          const pid = projectStore.currentProject?.id;
          const oid = orgStore.currentOrg?.id;
          if (oid && pid) router.push(`/orgs/${oid}/projects/${pid}/concepts`);
        },
      });
    }
  }

  return items.slice(0, 20);
});

// Group results for display
const groups = computed(() => {
  const map = new Map<string, ResultItem[]>();
  for (const item of results.value) {
    if (!map.has(item.group)) map.set(item.group, []);
    map.get(item.group)!.push(item);
  }
  return map;
});

// Flat list for keyboard nav
const flatItems = computed(() => results.value);

function open() {
  visible.value = true;
  query.value = '';
  selectedIdx.value = 0;
  nextTick(() => inputRef.value?.focus());
}

function close() {
  visible.value = false;
}

function onKeydown(e: KeyboardEvent) {
  if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
    e.preventDefault();
    visible.value ? close() : open();
  }
}

function onPaletteKey(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    close();
  } else if (e.key === 'ArrowDown') {
    e.preventDefault();
    selectedIdx.value = Math.min(selectedIdx.value + 1, flatItems.value.length - 1);
  } else if (e.key === 'ArrowUp') {
    e.preventDefault();
    selectedIdx.value = Math.max(selectedIdx.value - 1, 0);
  } else if (e.key === 'Enter') {
    const item = flatItems.value[selectedIdx.value];
    if (item) {
      item.action();
      close();
    }
  }
}

watch(query, () => {
  selectedIdx.value = 0;
});

onMounted(() => window.addEventListener('keydown', onKeydown));
onUnmounted(() => window.removeEventListener('keydown', onKeydown));
</script>

<template>
  <Teleport to="body">
    <div v-if="visible" class="palette-backdrop" @click.self="close" @keydown="onPaletteKey">
      <div class="palette">
        <div class="palette-input-wrap">
          <t-icon name="search" class="palette-icon" />
          <input
            ref="inputRef"
            v-model="query"
            class="palette-input"
            :placeholder="t('search.placeholder')"
            @keydown="onPaletteKey"
          />
          <kbd class="palette-esc" @click="close">Esc</kbd>
        </div>

        <div class="palette-results">
          <template v-if="flatItems.length === 0">
            <div class="palette-empty">{{ t('search.noResults') }}</div>
          </template>
          <template v-else>
            <template v-for="[group, items] in groups" :key="group">
              <div class="palette-group-label">{{ group }}</div>
              <button
                v-for="item in items"
                :key="item.id"
                class="palette-item"
                :class="{ 'is-selected': flatItems.indexOf(item) === selectedIdx }"
                @click="
                  item.action();
                  close();
                "
                @mouseenter="selectedIdx = flatItems.indexOf(item)"
              >
                <span class="palette-item__label">{{ item.label }}</span>
                <span v-if="item.desc" class="palette-item__desc">{{ item.desc }}</span>
              </button>
            </template>
          </template>
        </div>

        <div class="palette-footer">
          <span>{{ t('search.hint') }}</span>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.palette-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  backdrop-filter: blur(4px);
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding-top: 80px;
  z-index: 9999;
}

.palette {
  width: 560px;
  max-height: 480px;
  background: var(--ordo-bg-panel);
  border: 1px solid var(--ordo-border-color);
  border-radius: 12px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  box-shadow: 0 24px 48px rgba(0, 0, 0, 0.3);
}

.palette-input-wrap {
  display: flex;
  align-items: center;
  padding: 12px 16px;
  border-bottom: 1px solid var(--ordo-border-color);
  gap: 10px;
}

.palette-icon {
  color: var(--ordo-text-secondary);
  flex-shrink: 0;
}

.palette-input {
  flex: 1;
  background: transparent;
  border: none;
  outline: none;
  font-size: 14px;
  color: var(--ordo-text-primary);
  font-family: inherit;
}

.palette-input::placeholder {
  color: var(--ordo-text-tertiary);
}

.palette-esc {
  font-size: 10px;
  padding: 2px 6px;
  border: 1px solid var(--ordo-border-color);
  border-radius: 4px;
  color: var(--ordo-text-secondary);
  cursor: pointer;
  font-family: inherit;
}

.palette-results {
  flex: 1;
  overflow-y: auto;
  padding: 8px 0;
}

.palette-empty {
  padding: 32px 16px;
  text-align: center;
  color: var(--ordo-text-secondary);
  font-size: 13px;
}

.palette-group-label {
  padding: 6px 16px 4px;
  font-size: 10px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--ordo-text-tertiary);
}

.palette-item {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 16px;
  border: none;
  background: transparent;
  cursor: pointer;
  text-align: left;
  gap: 12px;
}

.palette-item.is-selected {
  background: rgba(255, 255, 255, 0.06);
}

.palette-item__label {
  font-size: 13px;
  color: var(--ordo-text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.palette-item__desc {
  font-size: 11px;
  color: var(--ordo-text-tertiary);
  white-space: nowrap;
  flex-shrink: 0;
}

.palette-footer {
  padding: 8px 16px;
  border-top: 1px solid var(--ordo-border-color);
  font-size: 11px;
  color: var(--ordo-text-tertiary);
}
</style>
