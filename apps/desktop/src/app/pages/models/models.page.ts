import {
  ChangeDetectionStrategy,
  Component,
  computed,
  effect,
  inject,
  signal
} from '@angular/core';
import type { ModelDescriptor } from '@lattice/types';
import { VoltButton } from '@voltui/components';

import { ModelRuntimeStore } from '../../core/state/model-runtime.store';
import { ModelSlotStore } from '../../core/state/model-slot.store';

@Component({
  selector: 'lat-models-page',
  imports: [VoltButton],
  templateUrl: './models.page.html',
  styleUrl: './models.page.css',
  changeDetection: ChangeDetectionStrategy.OnPush
})
export class ModelsPage {
  private readonly runtimeStore = inject(ModelRuntimeStore);
  private readonly slotStore = inject(ModelSlotStore);

  protected readonly status = this.runtimeStore.status;
  protected readonly error = this.runtimeStore.error;
  protected readonly isLoading = this.runtimeStore.isLoading;
  protected readonly isSaving = this.runtimeStore.isSaving;
  protected readonly isProbing = this.runtimeStore.isProbing;
  protected readonly canProbe = this.runtimeStore.canProbe;
  protected readonly isStarting = this.runtimeStore.isStarting;
  protected readonly isStopping = this.runtimeStore.isStopping;
  protected readonly canStart = this.runtimeStore.canStart;
  protected readonly canStop = this.runtimeStore.canStop;
  protected readonly isOperationPending = this.runtimeStore.isOperationPending;
  protected readonly executablePath = signal('');
  protected readonly canConfigure = computed(
    () => this.executablePath().trim().length > 0 && !this.isSaving()
  );
  protected readonly runtimeLabel = computed(() => {
    const status = this.status();
    if (status === null) {
      return 'Checking';
    }
    if (status.availability === 'running') {
      return 'Running';
    }
    if (status.executablePath === undefined) {
      return 'Not configured';
    }
    return capitalize(status.availability);
  });
  protected readonly runtimeIsRunning = computed(() => this.status()?.availability === 'running');

  protected readonly slotStatus = this.slotStore.status;
  protected readonly slotError = this.slotStore.error;
  protected readonly isSlotLoading = this.slotStore.isLoading;
  protected readonly isLoadingModel = this.slotStore.isLoadingModel;
  protected readonly isUnloading = this.slotStore.isUnloading;
  protected readonly canLoadModel = this.slotStore.canLoad;
  protected readonly canUnloadModel = this.slotStore.canUnload;
  protected readonly isModelOperationPending = this.slotStore.isOperationPending;
  protected readonly loadedModelLabel = computed(() => {
    const slot = this.slotStatus();
    if (slot?.ownership.state === 'owned') {
      return slot.ownership.modelKey;
    }
    if (slot?.loaded?.modelKey !== undefined) {
      return slot.loaded.modelKey;
    }
    return 'No model loaded';
  });

  constructor() {
    effect(() => {
      const path = this.status()?.executablePath;
      if (path !== undefined) {
        this.executablePath.set(path);
      }
    });

    effect(() => {
      // Re-read the model slot whenever runtime status changes, since an
      // installed/loaded inventory can only be listed while running.
      this.status();
      void this.slotStore.load();
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

  protected start(): void {
    void this.runtimeStore.start();
  }

  protected stop(): void {
    void this.runtimeStore.stop();
  }

  protected cancelOperation(): void {
    void this.runtimeStore.cancelOperation();
  }

  protected refresh(): void {
    void this.runtimeStore.load();
    void this.slotStore.load();
  }

  protected loadModel(modelKey: string): void {
    void this.slotStore.loadModel(modelKey);
  }

  protected unloadModel(): void {
    void this.slotStore.unloadModel();
  }

  protected cancelModelOperation(): void {
    void this.slotStore.cancelOperation();
  }

  protected formatBytes(bytes: number | undefined): string {
    if (bytes === undefined) {
      return 'Size unknown';
    }
    if (bytes < 1_000_000_000) {
      return `${Math.round(bytes / 1_000_000)} MB`;
    }
    return `${(bytes / 1_000_000_000).toFixed(1)} GB`;
  }

  protected modelSubtitle(model: ModelDescriptor): string {
    return `${model.architecture ?? 'Unknown architecture'} · ${this.formatBytes(model.sizeBytes)}`;
  }

  protected formatTime(unixSeconds: number | undefined): string {
    if (unixSeconds === undefined) {
      return 'Never';
    }
    if (unixSeconds < 946_684_800) {
      return 'Recently';
    }
    return new Intl.DateTimeFormat(undefined, {
      dateStyle: 'medium',
      timeStyle: 'short'
    }).format(new Date(unixSeconds * 1000));
  }
}

function capitalize(value: string): string {
  return `${value.slice(0, 1).toUpperCase()}${value.slice(1)}`;
}
