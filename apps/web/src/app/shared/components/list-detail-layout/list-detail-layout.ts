import { Component } from '@angular/core';

/**
 * Reusable split-panel layout: scrollable list on the left, detail panel on the right.
 *
 * Usage:
 *   <app-list-detail-layout>
 *     <div list>...list items...</div>
 *     <div detail class="w-[480px] shrink-0 ...">...detail content...</div>
 *   </app-list-detail-layout>
 *
 * The `[list]` slot is wrapped in a flex-1 min-w-0 container.
 * The `[detail]` slot is projected as-is (caller controls width/style).
 */
@Component({
  selector: 'app-list-detail-layout',
  standalone: true,
  template: `
    <div class="flex flex-col lg:flex-row gap-4 lg:gap-6">
      <div class="flex-1 min-w-0">
        <ng-content select="[list]" />
      </div>
      <ng-content select="[detail]" />
    </div>
  `,
})
export class ListDetailLayoutComponent {}
