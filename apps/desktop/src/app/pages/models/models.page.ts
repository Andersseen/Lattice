import {
  ChangeDetectionStrategy,
  Component,
  computed,
  effect,
  inject,
  signal
} from '@angular/core';
import { VoltButton } from '@voltui/components';

import { ModelRuntimeStore } from '../../core/state/model-runtime.store';

@Component({
  selector: 'lat-models-page',
  imports: [VoltButton],
  templateUrl: './models.page.html',
  styleUrl: './models.page.css',
  changeDetection: ChangeDetectionStrategy.OnPush
})
export class ModelsPage {
  private readonly runtimeStore = inject(ModelRuntimeStore);

  protected readonly status = this.runtimeStore.status;
  protected readonly error = this.runtimeStore.error;
  protected readonly isLoading = this.runtimeStore.isLoading;
  protected readonly isSaving = this.runtimeStore.isSaving;
  protected readonly isProbing = this.runtimeStore.isProbing;
  protected readonly canProbe = this.runtimeStore.canProbe;
  protected readonly executablePath = signal('');
  protected readonly canConfigure = computed(
    () => this.executablePath().trim().length > 0 && !this.isSaving()
  );

  constructor() {
    effect(() => {
      const path = this.status()?.executablePath;
      if (path !== undefined) {
        this.executablePath.set(path);
      }
    });
  }

  protected setExecutablePath(event: Event): void {
    const input = event.target as HTMLInputElement;
    this.executablePath.set(input.value);
  }

  protected configure(): void {
    if (!this.canConfigure()) {
      return;
    }

    void this.runtimeStore.configure(this.executablePath().trim());
  }

  protected probe(): void {
    void this.runtimeStore.probe();
  }

  protected refresh(): void {
    void this.runtimeStore.load();
  }
}
