import { Component, input, output } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { TranslocoModule } from '@jsverse/transloco';
import { SpTaskUpdate, UpdateKind } from '../../../../core/services/tasks-api.service';

const KIND_COLORS: Record<string, string> = {
  progress: 'text-ctp-blue latte:text-ctp-blue-900',
  blocker: 'text-ctp-red latte:text-ctp-red-900',
  question: 'text-ctp-yellow latte:text-ctp-yellow-950',
  artifact: 'text-ctp-green latte:text-ctp-green-950',
  note: 'text-ctp-overlay0 latte:text-ctp-subtext1',
};

/** Patterns that indicate an error in update content, regardless of kind. */
const ERROR_PATTERNS: RegExp[] = [
  /\berror[:\s]/i,
  /\bfailed\b/i,
  /\bfailure\b/i,
  /\bpanicked?\b/i,
  /\bexception\b/i,
  /\bfatal\b/i,
  /\bcrash(ed)?\b/i,
  /\bstderr\b/i,
  /\bnot found\b/i,
  /\btimed?\s*out\b/i,
];

function containsError(content: string): boolean {
  return ERROR_PATTERNS.some((pattern) => pattern.test(content));
}

@Component({
  selector: 'app-task-updates',
  standalone: true,
  imports: [TranslocoModule, FormsModule],
  template: `
    <div *transloco="let t">
      <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-2">{{ t('tasks.updates') }}</h3>

      <!-- Post update form -->
      <div class="flex gap-2 mb-3">
        <select [(ngModel)]="newKind"
          class="bg-surface text-text-primary text-xs rounded px-2 py-1.5 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent">
          @for (k of updateKinds; track k) {
            <option [value]="k">{{ k }}</option>
          }
        </select>
        <input type="text" [(ngModel)]="newContent" [placeholder]="t('tasks.updatePlaceholder')"
          class="flex-1 bg-surface text-text-primary text-xs rounded px-2 py-1.5 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary"
          (keydown.enter)="postUpdate()" />
        <button (click)="postUpdate()" [disabled]="!newContent.trim()"
          class="px-3 py-1.5 bg-accent text-bg rounded-lg text-xs font-medium hover:opacity-90 disabled:opacity-30">
          {{ t('tasks.post') }}
        </button>
      </div>

      <!-- Updates list -->
      @if (loading()) {
        <p class="text-text-muted text-xs">{{ t('common.loading') }}</p>
      } @else {
        <div class="space-y-1.5">
          @for (update of updates(); track update.id) {
            <div class="flex gap-2 text-xs" [class.opacity-90]="hasError(update)">
              <span class="text-text-muted shrink-0">{{ formatTime(update.created_at) }}</span>
              <span class="font-medium shrink-0 {{ kindColor(update.kind) }}">[{{ update.kind }}]</span>
              @if (hasError(update)) {
                <span class="text-ctp-red latte:text-ctp-red-900 break-words" title="Error detected in content">{{ update.content }}</span>
              } @else {
                <span class="text-text-primary break-words">{{ update.content }}</span>
              }
            </div>
          } @empty {
            <p class="text-text-muted text-xs">{{ t('tasks.noUpdates') }}</p>
          }
        </div>
      }
    </div>
  `,
})
export class TaskUpdatesComponent {
  updates = input.required<SpTaskUpdate[]>();
  loading = input(false);

  post = output<{ kind: string; content: string }>();

  readonly updateKinds: UpdateKind[] = ['progress', 'blocker', 'question', 'artifact', 'note'];
  newKind: UpdateKind = 'progress';
  newContent = '';

  kindColor(kind: string): string {
    return KIND_COLORS[kind] ?? 'text-text-secondary';
  }

  hasError(update: SpTaskUpdate): boolean {
    // blockers are already styled red by kind; check content for other kinds
    if (update.kind === 'blocker') return false;
    return containsError(update.content);
  }

  formatTime(iso: string): string {
    return iso?.substring(11, 16) ?? '??:??';
  }

  postUpdate(): void {
    const content = this.newContent.trim();
    if (!content) return;
    this.post.emit({ kind: this.newKind, content });
    this.newContent = '';
  }
}
