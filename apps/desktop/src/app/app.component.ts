import { ChangeDetectionStrategy, Component, inject } from '@angular/core';
import { RouterOutlet } from '@angular/router';

import { SettingsStore } from './core/state/settings.store';
import { ShellComponent } from './layout/shell.component';

@Component({
  selector: 'lat-root',
  imports: [RouterOutlet, ShellComponent],
  template: `
    <lat-shell>
      <router-outlet />
    </lat-shell>
  `,
  changeDetection: ChangeDetectionStrategy.OnPush
})
export class AppComponent {
  constructor() {
    inject(SettingsStore);
  }
}
