import { ChangeDetectionStrategy, Component, computed, inject } from '@angular/core';
import { RouterLink } from '@angular/router';
import { VoltButton } from '@voltui/components';
import { MoveAnimateDirective, MoveHoverDirective } from 'angular-movement';

import { AppInfoStore } from '../../core/state/app-info.store';

@Component({
  selector: 'lat-home-page',
  imports: [MoveAnimateDirective, MoveHoverDirective, RouterLink, VoltButton],
  templateUrl: './home.page.html',
  styleUrl: './home.page.css',
  changeDetection: ChangeDetectionStrategy.OnPush
})
export class HomePage {
  private readonly appInfoStore = inject(AppInfoStore);

  protected readonly appInfo = this.appInfoStore.appInfo;
  protected readonly error = this.appInfoStore.error;
  protected readonly isLoading = this.appInfoStore.isLoading;
  protected readonly runtimeLabel = computed(() => {
    const info = this.appInfo();
    return info ? `${info.runtime} / ${info.buildProfile}` : 'Checking runtime';
  });

  protected refresh(): void {
    void this.appInfoStore.refresh();
  }
}
