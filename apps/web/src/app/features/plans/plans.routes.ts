import { Routes } from '@angular/router';

export const plansRoutes: Routes = [
  {
    path: '',
    loadComponent: () => import('./pages/plan-list/plan-list').then(m => m.PlanListPage),
  },
  {
    path: ':id',
    loadComponent: () => import('./pages/plan-detail/plan-detail').then(m => m.PlanDetailPage),
  },
];
