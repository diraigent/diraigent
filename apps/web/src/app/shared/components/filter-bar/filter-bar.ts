import { Component, input, output } from '@angular/core';
import { FormsModule } from '@angular/forms';

/**
 * Reusable search/filter bar.
 * Renders a text search input and projects additional filter controls (dropdowns, etc.)
 * via content projection.
 *
 * Usage:
 *   <app-filter-bar
 *     [placeholder]="t('foo.searchPlaceholder')"
 *     [query]="searchQuery()"
 *     (queryChange)="searchQuery.set($event)">
 *     <select ...>...</select>
 *   </app-filter-bar>
 */
@Component({
  selector: 'app-filter-bar',
  standalone: true,
  imports: [FormsModule],
  template: `
    <div class="flex flex-wrap gap-3 mb-6">
      <input
        type="text"
        [placeholder]="placeholder()"
        [ngModel]="query()"
        (ngModelChange)="queryChange.emit($event)"
        class="flex-1 min-w-[200px] bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
               focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary" />
      <ng-content />
    </div>
  `,
})
export class FilterBarComponent {
  placeholder = input('Search...');
  query = input('');
  queryChange = output<string>();
}
