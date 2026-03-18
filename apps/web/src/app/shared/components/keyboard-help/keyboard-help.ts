import { Component, inject } from '@angular/core';
import { KeyboardService, ShortcutEntry } from '../../../core/services/keyboard.service';

/**
 * Keyboard shortcut help overlay.
 * Shown when the user presses `?`. Displays all registered shortcuts grouped by category.
 */
@Component({
  selector: 'app-keyboard-help',
  standalone: true,
  template: `
    <div
      class="fixed inset-0 bg-black/50 flex items-center justify-center z-[80]"
      role="dialog"
      aria-modal="true"
      aria-label="Keyboard shortcuts"
      (click)="close()"
      (keydown.escape)="close()">
      <div
        class="bg-bg border border-border rounded-xl p-6 w-full max-w-lg max-h-[80vh] overflow-y-auto"
        role="document"
        (click)="$event.stopPropagation()"
        (keydown)="$event.stopPropagation()">
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-lg font-semibold text-text-primary">Keyboard Shortcuts</h2>
          <button
            (click)="close()"
            class="text-text-secondary hover:text-text-primary p-1 rounded-lg hover:bg-bg-muted transition-colors"
            aria-label="Close">
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        @for (group of groups; track group) {
          <div class="mb-4">
            <h3 class="text-xs font-medium text-text-secondary uppercase tracking-wider mb-2">{{ group }}</h3>
            <div class="space-y-1">
              @for (shortcut of shortcutsByGroup(group); track shortcut.key) {
                <div class="flex items-center justify-between py-1.5 px-2 rounded-lg hover:bg-bg-muted">
                  <span class="text-sm text-text-primary">{{ shortcut.description }}</span>
                  <kbd class="inline-flex items-center justify-center min-w-[1.75rem] h-7 px-2
                              text-xs font-mono font-medium text-text-secondary
                              bg-bg-subtle border border-border rounded-md shadow-sm">
                    {{ shortcut.key }}
                  </kbd>
                </div>
              }
            </div>
          </div>
        }

        <p class="text-xs text-text-secondary mt-4 pt-3 border-t border-border">
          Shortcuts are disabled when typing in input fields or when a modal is open.
        </p>
      </div>
    </div>
  `,
})
export class KeyboardHelpComponent {
  private keyboard = inject(KeyboardService);

  /** All shortcuts from the service. */
  private allShortcuts: ShortcutEntry[] = this.keyboard.shortcuts;

  /** Unique group names in order. */
  groups: string[] = [...new Set(this.allShortcuts.map(s => s.group))];

  shortcutsByGroup(group: string): ShortcutEntry[] {
    return this.allShortcuts.filter(s => s.group === group);
  }

  close(): void {
    this.keyboard.helpOpen.set(false);
  }
}
