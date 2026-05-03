import { createRouter, createWebHistory } from 'vue-router';
import { useAuthStore } from '@/stores/auth';

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
        {
          path: 'servers',
          name: 'servers',
          component: () => import('@/views/server/ServerRegistryView.vue'),
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
        {
          path: 'orgs/:orgId/roles',
          name: 'org-roles',
          component: () => import('@/views/org/RolesView.vue'),
        },
        {
          path: 'orgs/:orgId/roles/new',
          name: 'org-role-create',
          component: () => import('@/views/org/RoleFormView.vue'),
        },
        {
          path: 'orgs/:orgId/roles/:roleId/edit',
          name: 'org-role-edit',
          component: () => import('@/views/org/RoleFormView.vue'),
        },

        // Projects list (org-scoped)
        {
          path: 'orgs/:orgId/projects',
          name: 'projects',
          component: () => import('@/views/project/ProjectListView.vue'),
        },

        // Server registry (org-scoped)
        {
          path: 'orgs/:orgId/servers',
          name: 'org-servers',
          component: () => import('@/views/server/ServerRegistryView.vue'),
        },

        // Notifications inbox
        {
          path: 'orgs/:orgId/notifications',
          name: 'notifications',
          component: () => import('@/views/notifications/NotificationsView.vue'),
        },

        // Marketplace
        {
          path: 'marketplace',
          name: 'marketplace',
          component: () => import('@/views/marketplace/MarketplaceView.vue'),
        },
        {
          path: 'marketplace/:owner/:repo',
          name: 'marketplace-detail',
          component: () => import('@/views/marketplace/MarketplaceDetailView.vue'),
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
              path: 'trace',
              name: 'project-trace',
              redirect: (to) => ({
                name: 'tests',
                params: to.params,
                query: to.query,
                hash: to.hash,
              }),
            },
            {
              path: 'releases',
              name: 'project-releases',
              component: () => import('@/views/project/ReleaseCenterView.vue'),
            },
            {
              path: 'releases/requests',
              name: 'project-release-requests',
              component: () => import('@/views/project/ReleaseRequestsView.vue'),
            },
            {
              path: 'releases/requests/new',
              name: 'project-release-request-create',
              component: () => import('@/views/project/CreateReleaseRequestView.vue'),
            },
            {
              path: 'releases/requests/:releaseId',
              name: 'project-release-request-detail',
              component: () => import('@/views/project/ReleaseRequestDetailView.vue'),
            },
            {
              path: 'releases/policies',
              name: 'project-release-policies',
              component: () => import('@/views/project/ReleasePoliciesView.vue'),
            },
            {
              path: 'releases/history',
              name: 'project-release-history',
              component: () => import('@/views/project/DeploymentsView.vue'),
            },
            {
              path: 'instances',
              name: 'project-instances',
              component: () => import('@/views/project/ProjectInstancesView.vue'),
            },
            {
              path: 'settings',
              name: 'project-settings',
              component: () => import('@/views/project/ProjectSettingsView.vue'),
            },
            {
              path: 'deployments',
              redirect: { name: 'project-release-history' },
            },
            {
              path: 'environments',
              name: 'project-environments',
              component: () => import('@/views/project/EnvironmentsView.vue'),
            },
            {
              path: 'environments/new',
              name: 'project-environment-create',
              component: () => import('@/views/project/EnvironmentFormView.vue'),
            },
            {
              path: 'environments/:envId/edit',
              name: 'project-environment-edit',
              component: () => import('@/views/project/EnvironmentFormView.vue'),
            },
          ],
        },
      ],
    },

    // Catch-all
    { path: '/:pathMatch(.*)*', redirect: '/' },
  ],
});

// Navigation guard: redirect to login if not authenticated
router.beforeEach((to) => {
  const auth = useAuthStore();
  if (!to.meta.public && !auth.isLoggedIn) {
    return { name: 'login', query: { redirect: to.fullPath } };
  }
  if (to.meta.public && auth.isLoggedIn && (to.name === 'login' || to.name === 'register')) {
    return { path: '/' };
  }
});

export default router;
