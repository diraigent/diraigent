import { Component, input } from '@angular/core';

/**
 * Reusable badge component for status/kind/severity/category labels.
 * Accepts a `color` class string (e.g. "bg-ctp-blue/20 text-ctp-blue") and a `label` to display.
 * Renders the standard pill badge: text-xs px-2 py-0.5 rounded-full font-medium.
 */
@Component({
  selector: 'app-status-badge',
  standalone: true,
  template: `<span class="px-2 py-0.5 rounded-full text-xs font-medium {{ color() }}">{{ label() }}</span>`,
})
export class StatusBadgeComponent {
  color = input.required<string>();
  label = input.required<string>();
}
