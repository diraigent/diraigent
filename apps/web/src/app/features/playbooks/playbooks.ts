import { Component, inject, signal, computed, ViewChild } from '@angular/core';
import { DatePipe, JsonPipe } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { Router } from '@angular/router';
import { TranslocoModule } from '@jsverse/transloco';
import { DiraigentApiService, DgProject } from '../../core/services/diraigent-api.service';
import {
  PlaybooksApiService,
  SpPlaybook,
} from '../../core/services/playbooks-api.service';
import { CrudFeatureBase } from '../../shared/crud-feature-base';
import { ConfirmDialogComponent } from '../../shared/components/confirm-dialog/confirm-dialog';
import { StepTemplatesPage } from './step-templates';

type PlaybooksTab = 'playbooks' | 'templates';

@Component({
  selector: 'app-playbooks',
  standalone: true,
  imports: [TranslocoModule, FormsModule, DatePipe, JsonPipe, StepTemplatesPage, ConfirmDialogComponent],
  template: `
    <div class="p-3 sm:p-6" *transloco="let t">
      <!-- Header -->
      <div class="flex items-center justify-between mb-3 sm:mb-6">
        <h1 class="text-2xl font-semibold text-text-primary">{{ t('nav.playbooks') }}</h1>
        <div class="flex items-center gap-2">
          @if (activeTab() === 'templates') {
            <button (click)="onCreateTemplate()" class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90">
              {{ t('stepTemplates.create') }}
            </button>
          } @else {
            <button (click)="navigateCreate()" class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90">
              {{ t('playbooks.create') }}
            </button>
          }
        </div>
      </div>

      <!-- Tab bar -->
      <div class="flex items-center gap-1 bg-surface rounded-lg border border-border p-1 mb-6 w-fit">
        <button
          (click)="activeTab.set('playbooks')"
          class="px-4 py-1.5 rounded-md text-sm font-medium transition-colors"
          [class.bg-bg]="activeTab() === 'playbooks'"
          [class.text-text-primary]="activeTab() === 'playbooks'"
          [class.shadow-sm]="activeTab() === 'playbooks'"
          [class.text-text-muted]="activeTab() !== 'playbooks'"
          [class.hover:text-text-secondary]="activeTab() !== 'playbooks'">
          {{ t('playbooks.tabPlaybooks') }}
        </button>
        <button
          (click)="activeTab.set('templates')"
          class="px-4 py-1.5 rounded-md text-sm font-medium transition-colors"
          [class.bg-bg]="activeTab() === 'templates'"
          [class.text-text-primary]="activeTab() === 'templates'"
          [class.shadow-sm]="activeTab() === 'templates'"
          [class.text-text-muted]="activeTab() !== 'templates'"
          [class.hover:text-text-secondary]="activeTab() !== 'templates'">
          {{ t('playbooks.tabTemplates') }}
        </button>
      </div>

      <!-- ── PLAYBOOKS TAB ── -->
      @if (activeTab() === 'playbooks') {
      <!-- Project default playbook selector -->
      <div class="mb-6 flex items-center gap-3">
        <label for="default-playbook-select" class="text-sm text-text-secondary whitespace-nowrap">{{ t('playbooks.projectDefault') }}:</label>
        <select
          id="default-playbook-select"
          [ngModel]="currentProject()?.default_playbook_id ?? ''"
          (ngModelChange)="setProjectDefault($event)"
          class="bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent min-w-[240px]">
          <option value="">{{ t('playbooks.noDefault') }}</option>
          @for (pb of items(); track pb.id) {
            <option [value]="pb.id">{{ pb.title }}</option>
          }
        </select>
      </div>

      <!-- Search -->
      <div class="mb-6">
        <input
          type="text"
          [placeholder]="t('playbooks.searchPlaceholder')"
          [ngModel]="searchQuery()"
          (ngModelChange)="searchQuery.set($event)"
          class="w-full max-w-md bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary" />
      </div>

      <!-- Accordion list -->
      <div class="space-y-2">
        @if (loading()) {
          <p class="text-text-secondary text-sm">{{ t('common.loading') }}</p>
        } @else if (filtered().length === 0) {
          <p class="text-text-secondary text-sm">{{ t('common.empty') }}</p>
        } @else {
          @for (pb of filtered(); track pb.id) {
            <div class="rounded-lg border transition-colors"
              [class]="pb.id === selected()?.id
                ? 'bg-accent/10 border-accent'
                : 'bg-surface border-border hover:border-accent/50'">
              <!-- Accordion header -->
              <button
                (click)="selectItem(pb)"
                class="w-full text-left p-4">
                <div class="flex items-center justify-between mb-1">
                  <div class="flex items-center gap-2 min-w-0">
                    <svg class="w-4 h-4 shrink-0 text-text-secondary transition-transform"
                      [class.rotate-90]="pb.id === selected()?.id"
                      fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" />
                    </svg>
                    <span class="text-sm font-medium text-text-primary truncate">{{ pb.title }}</span>
                    @if (!pb.tenant_id) {
                      <svg class="shrink-0 w-5 h-5" viewBox="0 0 100 100" fill="none" xmlns="http://www.w3.org/2000/svg" [attr.aria-label]="t('playbooks.defaultBadge')">
                        <title>{{ t('playbooks.defaultBadge') }}</title>
                        <circle cx="50" cy="50" r="50" fill="#2185D0"/>
                        <path d="M50 0 L52.5 33.38 L50 48.30 L47.5 33.38 L50 0 Z" fill="#f38ba8"/>
                        <path d="M2.44717 34.5491 H38.7743 L50 50 L61.2257 34.5491 H97.5528 L68.1636 55.9017 L79.3893 90 L50 69 L20.6107 90.4509 L31.8364 55.9017 Z" fill="white"/>
                      </svg>
                    }
                  </div>
                  <div class="flex items-center gap-2 shrink-0 ml-2">
                    <span class="text-xs text-text-secondary">
                      {{ pb.steps.length }} {{ pb.steps.length === 1 ? t('playbooks.step') : t('playbooks.steps') }}
                    </span>
                    @if (pb.trigger_description) {
                      <span class="hidden sm:inline text-xs text-text-muted">{{ pb.trigger_description }}</span>
                    }
                  </div>
                </div>
                @if (pb.tags.length > 0 && pb.id !== selected()?.id) {
                  <div class="flex flex-wrap gap-1 mt-1 ml-6">
                    @for (tag of pb.tags; track tag) {
                      <span class="px-1.5 py-0.5 bg-surface-hover text-text-secondary rounded text-xs">{{ tag }}</span>
                    }
                  </div>
                }
              </button>

              <!-- Accordion body (expanded details) -->
              @if (pb.id === selected()?.id) {
                <div class="border-t border-border px-4 sm:px-6 py-4">
                  <!-- Action buttons -->
                  <div class="flex items-center justify-between mb-4">
                    <div class="flex items-center gap-2">
                      @if (pb.trigger_description) {
                        <span class="text-sm text-text-secondary">{{ pb.trigger_description }}</span>
                      }
                    </div>
                    <div class="flex gap-2 shrink-0">
                      @if (!pb.tenant_id) {
                        <button (click)="clonePlaybook(pb); $event.stopPropagation()" class="px-3 py-1.5 text-xs bg-accent/10 text-accent rounded hover:bg-accent/20" [title]="t('playbooks.clone')">
                          {{ t('playbooks.clone') }}
                        </button>
                      } @else {
                        <button (click)="navigateEdit(pb); $event.stopPropagation()" class="p-1.5 text-text-secondary hover:text-accent rounded" [title]="t('playbooks.edit')">
                          <svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                            <path d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
                          </svg>
                        </button>
                        <button (click)="confirmDelete(pb); $event.stopPropagation()" class="p-1.5 text-text-secondary hover:text-ctp-red rounded" [title]="t('playbooks.delete')">
                          <svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                            <path d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                          </svg>
                        </button>
                      }
                    </div>
                  </div>

                  <!-- Tags -->
                  @if (pb.tags.length > 0) {
                    <div class="flex flex-wrap gap-1 mb-4">
                      @for (tag of pb.tags; track tag) {
                        <span class="px-2 py-0.5 bg-surface-hover text-text-secondary rounded text-xs">{{ tag }}</span>
                      }
                    </div>
                  }

                  <!-- Steps -->
                  <div class="border-t border-border pt-4 mb-4">
                    <h3 class="text-sm font-medium text-text-primary mb-3">{{ t('playbooks.stepsTitle') }}</h3>
                    <div class="space-y-3">
                      @for (step of pb.steps; track $index; let i = $index) {
                        <div class="bg-bg rounded-lg border border-border p-3">
                          <div class="flex items-center justify-between mb-1">
                            <div class="flex items-center gap-2">
                              <span class="w-6 h-6 bg-accent/20 text-accent rounded-full text-xs font-medium flex items-center justify-center">
                                {{ i + 1 }}
                              </span>
                              <span class="text-sm font-medium text-text-primary">{{ step.name }}</span>
                            </div>
                            @if (step.allowed_tools) {
                              <span class="text-xs px-2 py-0.5 rounded bg-surface-hover text-text-secondary">
                                {{ step.allowed_tools }}
                              </span>
                            }
                          </div>
                          @if (step.description) {
                            <p class="text-xs text-text-secondary mt-1 ml-8">{{ step.description }}</p>
                          }
                          @if (step.model || step.budget) {
                            <p class="text-xs text-text-muted mt-1 ml-8">
                              @if (step.model) { {{ step.model }} }
                              @if (step.model && step.budget) { · }
                              @if (step.budget) { {{ '$' + step.budget }} budget }
                            </p>
                          }
                        </div>
                      }
                    </div>
                  </div>

                  <!-- Metadata -->
                  @if (pb.metadata && objectKeys(pb.metadata).length > 0) {
                    <div class="border-t border-border pt-4 mb-4">
                      <h3 class="text-sm font-medium text-text-primary mb-2">{{ t('playbooks.metadata') }}</h3>
                      <pre class="text-xs text-text-secondary bg-bg rounded-lg border border-border p-3 overflow-x-auto">{{ pb.metadata | json }}</pre>
                    </div>
                  }

                  <div class="border-t border-border pt-3 text-xs text-text-secondary">
                    {{ t('playbooks.updatedAt') }}: {{ pb.updated_at | date:'medium' }}
                  </div>
                </div>
              }
            </div>
          }
        }
      </div>
      } <!-- end playbooks tab -->

      <!-- ── STEP TEMPLATES TAB ── -->
      @if (activeTab() === 'templates') {
        <app-step-templates #stepTemplates [embedded]="true" />
      }

      @if (showDeleteConfirm()) {
        <app-confirm-dialog
          [title]="t('playbooks.deleteConfirmTitle')"
          [message]="t('playbooks.deleteConfirmMessage')"
          [cancelLabel]="t('common.cancel')"
          [confirmLabel]="t('playbooks.delete')"
          (confirmed)="executeDelete()"
          (cancelled)="showDeleteConfirm.set(false)" />
      }
    </div>
  `,
})
export class PlaybooksPage extends CrudFeatureBase<SpPlaybook> {
  private api = inject(PlaybooksApiService);
  private projectApi = inject(DiraigentApiService);
  private router = inject(Router);

  activeTab = signal<PlaybooksTab>('playbooks');
  currentProject = signal<DgProject | null>(null);

  @ViewChild('stepTemplates') private stepTemplatesComp?: StepTemplatesPage;

  filtered = computed(() => {
    const q = this.searchQuery().toLowerCase().trim();
    if (!q) return this.items();
    return this.items().filter(
      pb => pb.title.toLowerCase().includes(q) || pb.trigger_description.toLowerCase().includes(q),
    );
  });

  override loadItems(): void {
    this.loading.set(true);
    this.loadCurrentProject();
    this.api.list().subscribe({
      next: (items) => this.refreshAfterMutation(items),
      error: () => this.loading.set(false),
    });
  }

  loadCurrentProject(): void {
    const projectId = this.ctx.projectId();
    if (!projectId) return;
    this.projectApi.getProjects().subscribe({
      next: (projects) => {
        const proj = projects.find(p => p.id === projectId);
        this.currentProject.set(proj ?? null);
      },
    });
  }

  protected override resetForm(): void {
    // Playbooks use router navigation for create/edit, no inline form
  }

  protected override fillForm(_item: SpPlaybook): void {
    // Playbooks use router navigation for create/edit, no inline form
  }

  setProjectDefault(playbookId: string): void {
    const projectId = this.ctx.projectId();
    if (!projectId) return;
    this.projectApi.updateProject(projectId, {
      default_playbook_id: playbookId || null,
    }).subscribe({
      next: (updated) => this.currentProject.set(updated),
    });
  }

  objectKeys(obj: Record<string, unknown>): string[] {
    return Object.keys(obj);
  }

  navigateCreate(): void {
    this.router.navigate(['/playbooks/create']);
  }

  navigateEdit(pb: SpPlaybook): void {
    this.router.navigate(['/playbooks', pb.id, 'edit']);
  }

  clonePlaybook(pb: SpPlaybook): void {
    // Navigate to builder in edit mode — the backend will transparently fork the default
    // into a tenant-owned copy when saved.
    this.router.navigate(['/playbooks', pb.id, 'edit']);
  }

  onCreateTemplate(): void {
    this.stepTemplatesComp?.openCreate();
  }

  showDeleteConfirm = signal(false);
  private deleteTarget: SpPlaybook | null = null;

  confirmDelete(pb: SpPlaybook): void {
    this.deleteTarget = pb;
    this.showDeleteConfirm.set(true);
  }

  executeDelete(): void {
    if (!this.deleteTarget) return;
    this.api.delete(this.deleteTarget.id).subscribe({
      next: () => {
        this.showDeleteConfirm.set(false);
        this.deleteTarget = null;
        this.selected.set(null);
        this.loadItems();
      },
      error: () => this.showDeleteConfirm.set(false),
    });
  }
}
