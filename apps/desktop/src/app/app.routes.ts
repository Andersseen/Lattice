import type { Routes } from '@angular/router';

export const routes: Routes = [
  {
    path: '',
    redirectTo: 'chat',
    pathMatch: 'full'
  },
  {
    path: 'system',
    redirectTo: 'settings'
  },
  {
    path: 'models',
    loadComponent: () => import('./pages/models/models.page')
  },
  {
    path: 'chat',
    loadComponent: () => import('./pages/chat/chat.page')
  },
  {
    path: 'history',
    loadComponent: () => import('./pages/history/history.page')
  },
  {
    path: 'settings',
    loadComponent: () => import('./pages/settings/settings.page')
  },
  {
    path: '**',
    redirectTo: ''
  }
];
