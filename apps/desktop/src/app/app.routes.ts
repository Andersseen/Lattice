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
    path: 'models',
    loadComponent: () => import('./pages/models/models.page').then((module) => module.ModelsPage)
  },
  {
    path: 'chat',
    loadComponent: () => import('./pages/chat/chat.page').then((module) => module.ChatPage)
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
