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
    loadComponent: () => import('./pages/models/models.page').then((module) => module.ModelsPage)
  },
  {
    path: 'chat',
    loadComponent: () => import('./pages/chat/chat.page').then((module) => module.ChatPage)
  },
  {
    path: 'history',
    loadComponent: () => import('./pages/history/history.page').then((module) => module.HistoryPage)
  },
  {
    path: 'settings',
    loadComponent: () =>
      import('./pages/settings/settings.page').then((module) => module.SettingsPage)
  },
  {
    path: '**',
    redirectTo: ''
  }
];
