import { ChangeDetectionStrategy, Component, computed, inject } from '@angular/core';
import { RouterLink, RouterLinkActive } from '@angular/router';
import { LmnSparklesIcon } from 'lumen-icons/sparkles';

import { ModelRuntimeStore } from '../core/state/model-runtime.store';
import { ModelSlotStore } from '../core/state/model-slot.store';

@Component({
  selector: 'lat-shell',
  imports: [LmnSparklesIcon, RouterLink, RouterLinkActive],
  templateUrl: './shell.component.html',
  styleUrl: './shell.component.css',
  changeDetection: ChangeDetectionStrategy.OnPush
})
export class ShellComponent {
  private readonly runtimeStore = inject(ModelRuntimeStore);
  private readonly slotStore = inject(ModelSlotStore);

  protected readonly runtimeStatus = this.runtimeStore.status;
  protected readonly slotStatus = this.slotStore.status;
  protected readonly statusLabel = computed(() => {
    const runtime = this.runtimeStatus();
    const slot = this.slotStatus();
    const modelKey = slot?.ownership.state === 'owned' ? slot.ownership.modelKey : null;

    if (runtime === null) {
      return 'Checking local AI';
    }

    if (runtime.availability !== 'running') {
      return 'Local AI stopped';
    }

    return modelKey === null ? 'Local AI · No model' : `Local AI · ${modelKey}`;
  });

  protected readonly statusTone = computed(() => {
    const runtime = this.runtimeStatus();
    const slot = this.slotStatus();
    const modelKey = slot?.ownership.state === 'owned' ? slot.ownership.modelKey : null;
    if (runtime?.availability === 'running' && modelKey !== null) {
      return 'ready';
    }
    if (runtime?.availability === 'running') {
      return 'partial';
    }
    return 'idle';
  });
}
