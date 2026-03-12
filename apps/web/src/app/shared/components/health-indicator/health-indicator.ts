import { Component, inject, signal } from '@angular/core';
import { DiraigentApiService } from '../../../core/services/diraigent-api.service';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { interval, startWith, switchMap, catchError, of } from 'rxjs';

@Component({
  selector: 'app-health-indicator',
  standalone: true,
  host: { class: 'flex items-center' },
  template: `
    <span class="inline-flex items-center gap-1 text-xs" [title]="healthy() ? 'API healthy' : 'API unreachable'">
      <span class="w-2 h-2 rounded-full" [class.bg-ctp-green]="healthy()" [class.bg-ctp-red]="!healthy()"></span>
    </span>
  `,
})
export class HealthIndicatorComponent {
  private api = inject(DiraigentApiService);

  healthy = signal(false);

  constructor() {
    interval(30_000).pipe(
      startWith(0),
      switchMap(() => this.api.getHealth().pipe(catchError(() => of(null)))),
      takeUntilDestroyed(),
    ).subscribe(res => this.healthy.set(res?.status === 'ok'));
  }
}
