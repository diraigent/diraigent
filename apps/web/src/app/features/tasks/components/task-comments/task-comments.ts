import { Component, input, output } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { TranslocoModule } from '@jsverse/transloco';
import { SpTaskComment } from '../../../../core/services/tasks-api.service';

@Component({
  selector: 'app-task-comments',
  standalone: true,
  imports: [TranslocoModule, FormsModule],
  template: `
    <div *transloco="let t">
      <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-2">{{ t('tasks.comments') }}</h3>

      <!-- Post comment form -->
      <div class="flex gap-2 mb-3">
        <input type="text" [(ngModel)]="newComment" [placeholder]="t('tasks.commentPlaceholder')"
          class="flex-1 bg-surface text-text-primary text-xs rounded px-2 py-1.5 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary"
          (keydown.enter)="postComment()" />
        <button (click)="postComment()" [disabled]="!newComment.trim()"
          class="px-3 py-1.5 bg-accent text-bg rounded-lg text-xs font-medium hover:opacity-90 disabled:opacity-30">
          {{ t('tasks.post') }}
        </button>
      </div>

      <!-- Comments list -->
      @if (loading()) {
        <p class="text-text-muted text-xs">{{ t('common.loading') }}</p>
      } @else {
        <div class="space-y-2 max-h-48 overflow-y-auto">
          @for (comment of comments(); track comment.id) {
            <div class="text-xs">
              <div class="flex items-center gap-2 mb-0.5">
                <span class="text-text-muted">{{ formatTime(comment.created_at) }}</span>
                <span class="font-medium text-ctp-mauve">{{ comment.agent_id ? 'assistant' : 'human' }}</span>
              </div>
              <p class="text-text-primary break-words">{{ comment.content }}</p>
            </div>
          } @empty {
            <p class="text-text-muted text-xs">{{ t('tasks.noComments') }}</p>
          }
        </div>
      }
    </div>
  `,
})
export class TaskCommentsComponent {
  comments = input.required<SpTaskComment[]>();
  loading = input(false);

  post = output<string>();

  newComment = '';

  formatTime(iso: string): string {
    return iso?.substring(11, 16) ?? '??:??';
  }

  postComment(): void {
    const content = this.newComment.trim();
    if (!content) return;
    this.post.emit(content);
    this.newComment = '';
  }
}
