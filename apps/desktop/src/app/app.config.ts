import { provideZonelessChangeDetection } from '@angular/core';
import type { ApplicationConfig } from '@angular/core';
import { provideRouter, withComponentInputBinding } from '@angular/router';
import { provideVoltTheme } from '@voltui/components';
import { provideMovement } from 'angular-movement';

import { routes } from './app.routes';

export const appConfig: ApplicationConfig = {
  providers: [
    provideZonelessChangeDetection(),
    provideRouter(routes, withComponentInputBinding()),
    provideVoltTheme({ color: 'sage', style: 'sharp', dark: false }),
    provideMovement({
      duration: 220,
      easing: 'cubic-bezier(0.16, 1, 0.3, 1)',
      disabled: false
    })
  ]
};
