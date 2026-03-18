import { Component, inject, input, output, signal, OnChanges } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { TranslocoModule } from '@jsverse/transloco';
import { SpTask, CreateTaskRequest, UpdateTaskRequest } from '../../../../core/services/tasks-api.service';
import { PlaybooksApiService, SpPlaybook } from '../../../../core/services/playbooks-api.service';
import { DiraigentApiService } from '../../../../core/services/diraigent-api.service';
import { ProjectContext } from '../../../../core/services/project-context.service';
import { DEFAULT_TASK_KINDS } from '../../../../shared/ui-constants';

@Component({
  selector: 'app-task-form',
  standalone: true,
  imports: [TranslocoModule, FormsModule],
  template: `
    @if (show()) {
      <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-[70]"
           role="button" tabindex="0" aria-label="Close modal"
           (click)="closeForm()" (keydown.enter)="closeForm()" (keydown.escape)="closeForm()" *transloco="let t">
        <div class="bg-bg border border-border rounded-xl p-6 w-full max-w-lg max-h-[90vh] overflow-y-auto"
             tabindex="-1" (click)="$event.stopPropagation()" (keydown.enter)="$event.stopPropagation()">
          <h2 class="text-lg font-semibold text-text-primary mb-4">
            {{ editing() ? t('tasks.editTitle') : t('tasks.createTitle') }}
          </h2>
          <div class="space-y-4">
            <div>
              <label for="tf-title" class="block text-sm text-text-secondary mb-1">{{ t('tasks.title') }}</label>
              <input id="tf-title" type="text" [(ngModel)]="formTitle"
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent" />
            </div>
            <div class="grid grid-cols-2 gap-4">
              <div>
                <label for="tf-kind" class="block text-sm text-text-secondary mb-1">{{ t('tasks.kind') }}</label>
                <select id="tf-kind" [(ngModel)]="formKind"
                  class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                         focus:outline-none focus:ring-1 focus:ring-accent">
                  @for (k of kinds(); track k) {
                    <option [value]="k">{{ k }}</option>
                  }
                </select>
              </div>
              <div class="flex items-end">
                <label for="tf-urgent" class="flex items-center gap-2 cursor-pointer select-none h-[42px]">
                  <input id="tf-urgent" type="checkbox" [(ngModel)]="formUrgent"
                    class="w-4 h-4 rounded border-border bg-surface text-ctp-red focus:ring-ctp-red focus:ring-1" />
                  <span class="text-sm text-text-primary">{{ t('tasks.urgent') }}</span>
                </label>
              </div>
            </div>
            <div>
              <label for="tf-spec" class="block text-sm text-text-secondary mb-1">{{ t('tasks.specField') }}</label>
              <textarea id="tf-spec" [(ngModel)]="formSpec" rows="4"
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent resize-y font-mono"></textarea>
            </div>
            <div>
              <label for="tf-acceptance" class="block text-sm text-text-secondary mb-1">{{ t('tasks.acceptanceCriteria') }}</label>
              <textarea id="tf-acceptance" [(ngModel)]="formAcceptanceCriteria" rows="3"
                [placeholder]="t('tasks.acceptanceCriteriaHint')"
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent resize-y font-mono"></textarea>
            </div>
            <div>
              <label for="tf-playbook" class="block text-sm text-text-secondary mb-1">{{ t('tasks.playbook') }}</label>
              <select id="tf-playbook" [(ngModel)]="formPlaybookId"
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent">
                <option value="">{{ t('tasks.noPlaybook') }}</option>
                @for (pb of playbooks(); track pb.id) {
                  <option [value]="pb.id">{{ pb.title }}</option>
                }
              </select>
            </div>
            @if (!editing()) {
              <label for="tf-decompose" class="flex items-center gap-2 cursor-pointer select-none">
                <input id="tf-decompose" type="checkbox" [(ngModel)]="formDecompose"
                  class="w-4 h-4 rounded border-border bg-surface text-accent focus:ring-accent focus:ring-1" />
                <span class="text-sm text-text-primary">{{ t('tasks.decompose') }}</span>
                <span class="text-xs text-text-muted">{{ t('tasks.decomposeHint') }}</span>
              </label>
            }
            <div class="flex justify-end gap-3 pt-2">
              <button (click)="closeForm()" class="px-4 py-2 text-sm text-text-secondary hover:text-text-primary">
                {{ t('tasks.cancel') }}
              </button>
              <button (click)="submitForm()" [disabled]="!formTitle.trim()"
                class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90 disabled:opacity-50">
                {{ editing() ? t('tasks.save') : t('tasks.create') }}
              </button>
            </div>
          </div>
        </div>
      </div>
    }
  `,
})
export class TaskFormComponent implements OnChanges {
  private playbooksApi = inject(PlaybooksApiService);
  private projectApi = inject(DiraigentApiService);
  private ctx = inject(ProjectContext);

  show = input(false);
  editing = input<SpTask | null>(null);

  submitCreate = output<CreateTaskRequest>();
  submitUpdate = output<{ id: string; data: UpdateTaskRequest }>();
  closed = output<void>();

  /** Dynamic kinds loaded from the project's package; falls back to DEFAULT_TASK_KINDS. */
  kinds = signal<string[]>(DEFAULT_TASK_KINDS);

  playbooks = signal<SpPlaybook[]>([]);
  private defaultPlaybookId = '';

  /** Cache: projectId → allowed task kinds to avoid redundant fetches. */
  private packageKindsCache = new Map<string, string[]>();

  formTitle = '';
  formKind = 'feature';
  formUrgent = false;
  formSpec = '';
  formAcceptanceCriteria = '';
  formPlaybookId = '';
  formDecompose = false;

  ngOnChanges(): void {
    const task = this.editing();
    if (task) {
      this.formTitle = task.title;
      this.formKind = task.kind || 'feature';
      this.formUrgent = task.urgent;
      this.formSpec = (task.context?.['spec'] as string) ?? '';
      const criteria = task.context?.['acceptance_criteria'] as string[] | undefined;
      this.formAcceptanceCriteria = criteria?.join('\n') ?? '';
      this.formPlaybookId = task.playbook_id ?? '';
      this.loadPlaybooks();
    } else if (this.show()) {
      this.formTitle = '';
      this.formKind = 'feature';
      this.formUrgent = false;
      this.formSpec = '';
      this.formAcceptanceCriteria = '';
      this.formPlaybookId = this.defaultPlaybookId;
      this.formDecompose = false;
      this.loadPlaybooks();
    }
  }

  private loadPlaybooks(): void {
    this.playbooksApi.list().subscribe({
      next: (items) => this.playbooks.set(items),
    });

    const projectId = this.ctx.projectId();
    if (!projectId) return;

    // Apply cached kinds immediately if available
    const cached = this.packageKindsCache.get(projectId);
    if (cached) {
      this.kinds.set(cached);
    }

    this.projectApi.getProject(projectId).subscribe({
      next: (proj) => {
        this.defaultPlaybookId = proj.default_playbook_id ?? '';
        if (!this.editing()) {
          this.formPlaybookId = this.defaultPlaybookId;
        }

        if (proj.package?.id) {
          this.loadPackageKinds(projectId, proj.package.id);
        } else {
          this.applyKinds(projectId, DEFAULT_TASK_KINDS);
        }
      },
      error: () => {
        // Fall back to defaults on project fetch failure
        this.applyKinds(projectId, DEFAULT_TASK_KINDS);
      },
    });
  }

  private loadPackageKinds(projectId: string, packageId: string): void {
    this.projectApi.getPackage(packageId).subscribe({
      next: (pkg) => {
        const taskKinds = pkg.allowed_task_kinds.length > 0
          ? pkg.allowed_task_kinds
          : DEFAULT_TASK_KINDS;
        this.applyKinds(projectId, taskKinds);
      },
      error: () => {
        // Fall back to defaults if package fetch fails
        this.applyKinds(projectId, DEFAULT_TASK_KINDS);
      },
    });
  }

  private applyKinds(projectId: string, kinds: string[]): void {
    this.packageKindsCache.set(projectId, kinds);
    this.kinds.set(kinds);

    // Ensure formKind is valid for the loaded kinds (only for new tasks)
    if (!this.editing() && !kinds.includes(this.formKind)) {
      this.formKind = kinds[0] ?? 'feature';
    }
  }

  closeForm(): void {
    this.closed.emit();
  }

  submitForm(): void {
    if (!this.formTitle.trim()) return;
    const task = this.editing();
    const context: Record<string, unknown> = {};
    if (this.formSpec.trim()) context['spec'] = this.formSpec.trim();
    const acLines = this.formAcceptanceCriteria
      .split('\n')
      .map((l) => l.trim())
      .filter((l) => l.length > 0);
    if (acLines.length > 0) context['acceptance_criteria'] = acLines;

    if (task) {
      this.submitUpdate.emit({
        id: task.id,
        data: {
          title: this.formTitle.trim(),
          kind: this.formKind,
          urgent: this.formUrgent,
          context: Object.keys(context).length > 0 ? { ...task.context, ...context } : task.context,
        },
      });
    } else {
      if (this.formDecompose) context['decompose'] = true;
      const req: CreateTaskRequest = {
        title: this.formTitle.trim(),
        kind: this.formKind,
        urgent: this.formUrgent,
      };
      if (Object.keys(context).length > 0) req.context = context;
      if (this.formPlaybookId.trim()) req.playbook_id = this.formPlaybookId.trim();
      this.submitCreate.emit(req);
    }
  }
}
