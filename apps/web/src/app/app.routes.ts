import { Routes } from '@angular/router';
import { AuthGuard } from './core/guards/auth.guard';
import { AuthCallbackPage } from './features/auth/pages/auth-callback/auth-callback';

export const routes: Routes = [
  {
    path: '',
    loadComponent: () => import('./features/landing/landing').then(m => m.LandingPage),
  },
  {
    path: 'dashboard',
    canActivate: [AuthGuard],
    loadComponent: () => import('./features/dashboard/dashboard').then(m => m.DashboardPage),
  },
  {
    path: 'work',
    canActivate: [AuthGuard],
    loadComponent: () => import('./features/work/work').then(m => m.WorkPage),
  },
  {
    path: 'review',
    canActivate: [AuthGuard],
    loadComponent: () => import('./features/review/review').then(m => m.ReviewPage),
  },
  {
    path: 'agents',
    redirectTo: 'settings',
    pathMatch: 'full',
  },
  {
    path: 'knowledge',
    canActivate: [AuthGuard],
    loadComponent: () => import('./features/knowledge/knowledge').then(m => m.KnowledgePage),
  },
  {
    path: 'decisions',
    canActivate: [AuthGuard],
    loadComponent: () => import('./features/decisions/decisions').then(m => m.DecisionsPage),
  },
  {
    path: 'playbooks',
    canActivate: [AuthGuard],
    loadComponent: () => import('./features/playbooks/playbooks').then(m => m.PlaybooksPage),
  },
  {
    path: 'playbooks/create',
    canActivate: [AuthGuard],
    loadComponent: () => import('./features/playbooks/playbook-builder').then(m => m.PlaybookBuilderPage),
  },
  {
    path: 'playbooks/:id/edit',
    canActivate: [AuthGuard],
    loadComponent: () => import('./features/playbooks/playbook-builder').then(m => m.PlaybookBuilderPage),
  },
  {
    path: 'step-templates',
    redirectTo: 'playbooks',
    pathMatch: 'full',
  },
  {
    path: 'goals',
    redirectTo: 'work',
    pathMatch: 'full',
  },
  {
    path: 'observations',
    redirectTo: 'review',
    pathMatch: 'full',
  },
  {
    path: 'reports',
    canActivate: [AuthGuard],
    loadComponent: () => import('./features/reports/reports').then(m => m.ReportsPage),
  },
  {
    path: 'team',
    redirectTo: 'settings',
    pathMatch: 'full',
  },
  {
    path: 'pipelines',
    canActivate: [AuthGuard],
    loadComponent: () => import('./features/pipelines/pipelines').then(m => m.PipelinesPage),
  },
  {
    path: 'pipelines/setup',
    canActivate: [AuthGuard],
    loadComponent: () => import('./features/pipelines/forgejo-setup').then(m => m.ForgejoSetupPage),
  },
  {
    path: 'pipelines/github-setup',
    canActivate: [AuthGuard],
    loadComponent: () => import('./features/pipelines/github-setup').then(m => m.GitHubSetupPage),
  },
  {
    path: 'pipelines/:runId',
    canActivate: [AuthGuard],
    loadComponent: () => import('./features/pipelines/run-detail').then(m => m.RunDetailPage),
  },
  {
    path: 'integrations',
    canActivate: [AuthGuard],
    loadComponent: () => import('./features/integrations/integrations').then(m => m.IntegrationsPage),
    children: [
      {
        path: '',
        loadComponent: () =>
          import('./features/integrations/pages/integration-list/integration-list').then(m => m.IntegrationListPage),
      },
      {
        path: 'new',
        loadComponent: () =>
          import('./features/integrations/pages/integration-form/integration-form').then(m => m.IntegrationFormPage),
      },
      {
        path: 'logs',
        loadComponent: () => import('./features/logs/logs').then(m => m.LogsPage),
      },
      {
        path: ':id',
        loadComponent: () =>
          import('./features/integrations/pages/integration-detail/integration-detail').then(
            m => m.IntegrationDetailPage,
          ),
      },
      {
        path: ':id/edit',
        loadComponent: () =>
          import('./features/integrations/pages/integration-form/integration-form').then(m => m.IntegrationFormPage),
      },
    ],
  },
  {
    path: 'logs',
    redirectTo: 'integrations/logs',
    pathMatch: 'full',
  },
  {
    path: 'verifications',
    canActivate: [AuthGuard],
    loadComponent: () => import('./features/verifications/verifications').then(m => m.VerificationsPage),
  },
  {
    path: 'source',
    canActivate: [AuthGuard],
    loadComponent: () => import('./features/source/source').then(m => m.SourcePage),
  },
  {
    path: 'audit',
    canActivate: [AuthGuard],
    loadComponent: () => import('./features/audit/audit').then(m => m.AuditPage),
  },
  {
    path: 'settings',
    canActivate: [AuthGuard],
    loadComponent: () => import('./features/settings/settings').then(m => m.SettingsPage),
  },
  {
    path: 'tenant-settings',
    canActivate: [AuthGuard],
    loadComponent: () => import('./features/tenant-settings/tenant-settings').then(m => m.TenantSettingsPage),
  },
  { path: 'auth/callback', component: AuthCallbackPage },
  {
    path: 'auth/logout',
    loadComponent: () => import('./features/auth/pages/logout/logout').then(m => m.LogoutPage),
  },
  { path: '**', redirectTo: '' },
];
