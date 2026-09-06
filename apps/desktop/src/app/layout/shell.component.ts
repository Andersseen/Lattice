import { ChangeDetectionStrategy, Component } from '@angular/core';
import { RouterLink, RouterLinkActive } from '@angular/router';
import { LmnSparklesIcon } from 'lumen-icons/sparkles';
import { TooltipDirective } from 'quartz-headless';

@Component({
  selector: 'lat-shell',
  imports: [LmnSparklesIcon, RouterLink, RouterLinkActive, TooltipDirective],
  templateUrl: './shell.component.html',
  styleUrl: './shell.component.css',
  changeDetection: ChangeDetectionStrategy.OnPush
})
export class ShellComponent {}
