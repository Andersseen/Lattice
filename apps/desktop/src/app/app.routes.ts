import type { Routes } from '@angular/router';

export const routes: Routes = [
  {
    path: '',
    loadComponent: () => import('./pages/home/home.page').then((module) => module.HomePage)
  },
  {
    path: 'system',
    loadComponent: () => import('./pages/system/system.page').then((module) => module.SystemPage)
  },
  {
    path: '**',
    redirectTo: ''
  }
];
