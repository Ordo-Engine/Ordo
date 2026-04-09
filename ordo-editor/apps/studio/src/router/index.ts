import { createRouter, createWebHistory } from 'vue-router'
import { useAuthStore } from '@/stores/auth'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    // ── Auth (no login required) ──────────────────────────────────────────────
    {
      path: '/login',
      name: 'login',
      component: () => import('@/views/auth/LoginView.vue'),
      meta: { public: true },
    },
    {
      path: '/register',
      name: 'register',
      component: () => import('@/views/auth/RegisterView.vue'),
      meta: { public: true },
    },

    // ── App shell (requires login) ────────────────────────────────────────────
    {
      path: '/',
      component: () => import('@/components/layout/AppLayout.vue'),
      children: [
        { path: '', redirect: '/dashboard' },

        // Dashboard
        {
          path: 'dashboard',
          name: 'dashboard',
          component: () => import('@/views/dashboard/DashboardView.vue'),
        },

        // User settings
        {
          path: 'settings',
          name: 'settings',
          component: () => import('@/views/settings/SettingsView.vue'),
        },

        // Organizations
        {
          path: 'orgs',
          name: 'orgs',
          component: () => import('@/views/org/OrgListView.vue'),
        },
        {
          path: 'orgs/:orgId/settings',
          name: 'org-settings',
          component: () => import('@/views/org/OrgSettingsView.vue'),
        },
        {
          path: 'orgs/:orgId/members',
          name: 'org-members',
          component: () => import('@/views/org/MembersView.vue'),
        },

        // Projects list (org-scoped)
        {
          path: 'orgs/:orgId/projects',
          name: 'projects',
          component: () => import('@/views/project/ProjectListView.vue'),
        },

        // Project sub-pages (org-scoped)
        {
          path: 'orgs/:orgId/projects/:projectId',
          component: () => import('@/components/layout/ProjectLayout.vue'),
          children: [
            { path: '', redirect: 'editor' },
            {
              path: 'editor',
              name: 'editor',
              component: () => import('@/views/editor/EditorView.vue'),
            },
            {
              path: 'editor/:rulesetName',
              name: 'editor-ruleset',
              component: () => import('@/views/editor/EditorView.vue'),
            },
            {
              path: 'facts',
              name: 'facts',
              component: () => import('@/views/project/FactCatalogView.vue'),
            },
            {
              path: 'concepts',
              name: 'concepts',
              component: () => import('@/views/project/ConceptRegistryView.vue'),
            },
            {
              path: 'contracts',
              name: 'contracts',
              component: () => import('@/views/project/ContractView.vue'),
            },
            {
              path: 'tests',
              name: 'tests',
              component: () => import('@/views/project/TestView.vue'),
            },
            {
              path: 'versions',
              name: 'versions',
              component: () => import('@/views/project/VersionHistoryView.vue'),
            },
            {
              path: 'settings',
              name: 'project-settings',
              component: () => import('@/views/project/ProjectSettingsView.vue'),
            },
          ],
        },
      ],
    },

    // Catch-all
    { path: '/:pathMatch(.*)*', redirect: '/' },
  ],
})

// Navigation guard: redirect to login if not authenticated
router.beforeEach((to) => {
  const auth = useAuthStore()
  if (!to.meta.public && !auth.isLoggedIn) {
    return { name: 'login', query: { redirect: to.fullPath } }
  }
  if (to.meta.public && auth.isLoggedIn && (to.name === 'login' || to.name === 'register')) {
    return { path: '/' }
  }
})

export default router
