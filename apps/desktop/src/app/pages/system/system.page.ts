import { ChangeDetectionStrategy, Component } from '@angular/core';

@Component({
  selector: 'lat-system-page',
  templateUrl: './system.page.html',
  styleUrl: './system.page.css',
  changeDetection: ChangeDetectionStrategy.OnPush
})
export class SystemPage {}
