import { Component, input, signal, inject, OnChanges, SimpleChanges } from '@angular/core';
import { TranslocoModule } from '@jsverse/transloco';
import { DomSanitizer, SafeHtml } from '@angular/platform-browser';
import { TasksApiService, ChangedFileSummary, ChangedFile } from '../../../../core/services/tasks-api.service';
import { ModalWrapperComponent } from '../../../../shared/components/modal-wrapper/modal-wrapper';
import * as Diff2Html from 'diff2html';

const CHANGE_BADGE: Record<string, { bg: string; text: string; label: string }> = {
  added: { bg: 'bg-ctp-green/15', text: 'text-ctp-green', label: 'A' },
  modified: { bg: 'bg-ctp-yellow/15', text: 'text-ctp-yellow', label: 'M' },
  deleted: { bg: 'bg-ctp-red/15', text: 'text-ctp-red', label: 'D' },
};

@Component({
  selector: 'app-changed-files',
  standalone: true,
  imports: [TranslocoModule, ModalWrapperComponent],
  styles: `
    :host {
      --d2h-del-bg-color: color-mix(in srgb, var(--catppuccin-color-red) 15%, var(--color-bg));
      --d2h-ins-bg-color: color-mix(in srgb, var(--catppuccin-color-green) 15%, var(--color-bg));
      --d2h-del-highlight-bg-color: color-mix(in srgb, var(--catppuccin-color-red) 50%, var(--color-bg));
      --d2h-ins-highlight-bg-color: color-mix(in srgb, var(--catppuccin-color-green) 50%, var(--color-bg));

      --d2h-dark-del-bg-color: color-mix(in srgb, var(--catppuccin-color-red) 15%, var(--color-bg));
      --d2h-dark-ins-bg-color: color-mix(in srgb, var(--catppuccin-color-green) 15%, var(--color-bg));
      --d2h-dark-del-highlight-bg-color: color-mix(in srgb, var(--catppuccin-color-red) 50%, var(--color-bg));
      --d2h-dark-ins-highlight-bg-color: color-mix(in srgb, var(--catppuccin-color-green) 50%, var(--color-bg));
    }

    :host ::ng-deep .d2h-wrapper {
      font-size: 12px;
    }
    :host ::ng-deep .d2h-file-header {
      display: none;
    }
    :host ::ng-deep .d2h-file-wrapper {
      border: none;
      margin: 0;
    }
    :host ::ng-deep .d2h-files-diff {
      width: 100%;
    }
    :host ::ng-deep .d2h-code-side-linenumber {
      width: 40px;
      min-width: 40px;
    }
    :host ::ng-deep table.d2h-diff-table {
      font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, monospace;
      font-size: 11px;
    }
    :host ::ng-deep .d2h-code-line-ctn {
      word-break: break-all;
      white-space: pre-wrap;
    }
    :host ::ng-deep .d2h-file-side-diff {
      overflow-x: auto;
    }
    /* Theme-aware overrides */
    :host ::ng-deep .d2h-file-diff {
      background: var(--color-bg);
    }
    :host ::ng-deep .d2h-code-line {
      color: var(--color-text-primary);
    }
    :host ::ng-deep .d2h-code-side-linenumber {
      color: var(--color-text-muted);
      background: var(--color-surface);
    }
    /* Deletion line — colored background, standard text */
    :host ::ng-deep .d2h-del {
      color: var(--ctp-text);
    }

    /* Addition line — colored background, standard text */
    :host ::ng-deep .d2h-ins {
      color: var(--ctp-text);
    }
    :host ::ng-deep .d2h-info {
      background: var(--color-surface-hover);
      color: var(--color-text-secondary);
    }
  `,
  template: `
    <div *transloco="let t">
      <!-- File list -->
      <div class="mb-3">
        <div class="flex items-center justify-between mb-2">
          <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider">Changed Files</h3>
          <span class="text-[10px] text-text-muted">{{ files().length }} file(s)</span>
        </div>

        @if (files().length === 0) {
          <p class="text-xs text-text-muted italic">No changed files recorded</p>
        } @else {
          <div class="space-y-0.5">
            @for (file of files(); track file.id) {
              <button
                (click)="toggleFile(file)"
                class="w-full flex items-center gap-2 px-2 py-1.5 rounded-lg text-left transition-colors
                       hover:bg-surface-hover group"
                [class.bg-surface-hover]="selectedFileId() === file.id">
                <!-- Change type badge -->
                <span class="inline-flex items-center justify-center w-5 h-5 rounded text-[10px] font-bold shrink-0
                             {{ badgeClasses(file.change_type).bg }} {{ badgeClasses(file.change_type).text }}">
                  {{ badgeClasses(file.change_type).label }}
                </span>
                <!-- File path -->
                <span class="text-xs font-mono text-text-primary truncate flex-1">{{ file.path }}</span>
                <!-- Open indicator -->
                <svg class="w-3 h-3 text-text-muted shrink-0"
                     fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                  <path d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
                </svg>
              </button>
            }
          </div>
        }
      </div>
    </div>

    <!-- Diff modal -->
    @if (selectedFileId()) {
      <app-modal-wrapper (closed)="closePanel()" maxWidth="max-w-7xl">
        <div class="-m-6 max-h-[calc(90vh-3rem)] flex flex-col overflow-hidden rounded-xl">
          <!-- Modal header -->
          <div class="flex items-center justify-between px-4 py-3 border-b border-border bg-surface shrink-0">
            <div class="flex items-center gap-2 min-w-0">
              <span class="inline-flex items-center justify-center w-5 h-5 rounded text-[10px] font-bold shrink-0
                           {{ selectedFileBadge().bg }} {{ selectedFileBadge().text }}">
                {{ selectedFileBadge().label }}
              </span>
              <span class="text-sm font-mono text-text-primary truncate">{{ selectedFilePath() }}</span>
            </div>
            <button (click)="closePanel()"
              class="p-1.5 text-text-muted hover:text-text-primary rounded-lg hover:bg-surface-hover transition-colors">
              <svg class="w-5 h-5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                <path d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>
          <!-- Modal body -->
          <div class="flex-1 overflow-auto">
            @if (diffLoading()) {
              <div class="p-8 text-sm text-text-muted text-center">Loading diff...</div>
            } @else if (!diffHtml()) {
              <div class="p-8 text-sm text-text-muted text-center italic">No diff available</div>
            } @else {
              <div class="overflow-x-auto" [innerHTML]="diffHtml()"></div>
            }
          </div>
        </div>
      </app-modal-wrapper>
    }
  `,
})
export class ChangedFilesComponent implements OnChanges {
  taskId = input.required<string>();
  files = input<ChangedFileSummary[]>([]);

  selectedFileId = signal<string | null>(null);
  diffHtml = signal<SafeHtml | null>(null);
  diffLoading = signal(false);

  private api = inject(TasksApiService);
  private sanitizer = inject(DomSanitizer);

  ngOnChanges(changes: SimpleChanges): void {
    if (changes['taskId']) {
      this.closePanel();
    }
  }

  badgeClasses(changeType: string): { bg: string; text: string; label: string } {
    return CHANGE_BADGE[changeType] ?? { bg: 'bg-ctp-overlay0/15', text: 'text-ctp-overlay0', label: '?' };
  }

  toggleFile(file: ChangedFileSummary): void {
    if (this.selectedFileId() === file.id) {
      this.closePanel();
      return;
    }
    this.selectedFileId.set(file.id);
    this.diffHtml.set(null);
    this.diffLoading.set(true);
    this.api.getChangedFile(this.taskId(), file.id).subscribe({
      next: (cf: ChangedFile) => {
        if (cf.diff) {
          const html = Diff2Html.html(cf.diff, {
            outputFormat: 'side-by-side',
            drawFileList: false,
            matching: 'lines',
            diffStyle: 'word',
          });
          this.diffHtml.set(this.sanitizer.bypassSecurityTrustHtml(html));
        }
        this.diffLoading.set(false);
      },
      error: () => {
        this.diffLoading.set(false);
      },
    });
  }

  selectedFilePath(): string {
    const id = this.selectedFileId();
    return this.files().find(f => f.id === id)?.path ?? '';
  }

  selectedFileBadge(): { bg: string; text: string; label: string } {
    const id = this.selectedFileId();
    const file = this.files().find(f => f.id === id);
    return file ? this.badgeClasses(file.change_type) : { bg: '', text: '', label: '' };
  }

  closePanel(): void {
    this.selectedFileId.set(null);
    this.diffHtml.set(null);
  }
}
