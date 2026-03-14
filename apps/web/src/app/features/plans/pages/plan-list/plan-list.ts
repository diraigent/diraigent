import { Component, inject, signal, computed } from '@angular/core';
import { DatePipe } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { Router } from '@angular/router';
import { TranslocoModule } from '@jsverse/transloco';
import { forkJoin, of } from 'rxjs';
import { catchError } from 'rxjs/operators';
import {
  PlansApiService,
  SpPlan,
  PlanStatus,
  SpPlanProgress,
} from '../../../../core/services/plans-api.service';
import { CrudFeatureBase } from '../../../../shared/crud-feature-base';
import { ModalWrapperComponent } from '../../../../shared/components/modal-wrapper/modal-wrapper';
import { FilterBarComponent } from '../../../../shared/components/filter-bar/filter-bar';

const STATUSES: PlanStatus[] = ['active', 'completed', 'cancelled'];

const STATUS_COLORS: Record<PlanStatus, string> = {
  active: 'bg-ctp-green/20 text-ctp-green',
  completed: 'bg-ctp-blue/20 text-ctp-blue',
  cancelled: 'bg-ctp-overlay0/20 text-ctp-overlay0',
};

const PROGRESS_COLORS: Record<PlanStatus, string> = {
  active: 'bg-ctp-green',
  completed: 'bg-ctp-blue',
  cancelled: 'bg-ctp-overlay0',
};

@Component({
  selector: 'app-plan-list',
  standalone: true,
  imports: [TranslocoModule, FormsModule, DatePipe, ModalWrapperComponent, FilterBarComponent],
  template: `
    <div class="p-3 sm:p-6" *transloco="let t">
      <!-- Header -->
      <div class="flex items-center justify-between mb-3 sm:mb-6">
        <h1 class="text-2xl font-semibold text-text-primary">{{ t('nav.plans') }}</h1>
        <button (click)="openCreate()" class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90">
          {{ t('plans.create') }}
        </button>
      </div>

      <!-- Filters -->
      <app-filter-bar
        [placeholder]="t('plans.searchPlaceholder')"
        [query]="searchQuery()"
        (queryChange)="searchQuery.set($event)">
        <select
          [(ngModel)]="selectedStatus"
          (ngModelChange)="loadItems()"
          class="bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent">
          <option value="">{{ t('plans.allStatuses') }}</option>
          @for (s of statuses; track s) {
            <option [value]="s">{{ t('plans.status.' + s) }}</option>
          }
        </select>
      </app-filter-bar>

      <!-- Content: accordion list -->
      @if (loading()) {
        <p class="text-text-secondary text-sm">{{ t('common.loading') }}</p>
      } @else if (filtered().length === 0) {
        <p class="text-text-secondary text-sm">{{ t('common.empty') }}</p>
      } @else {
        <div class="space-y-2">
          @for (item of filtered(); track item.id) {
            <div class="rounded-lg border transition-colors"
              [class]="item.id === selected()?.id
                ? 'bg-accent/10 border-accent'
                : 'bg-surface border-border hover:border-accent/50'"
              [class.border-l-4]="item.status === 'active'"
              [class.border-l-ctp-green]="item.status === 'active'">
              <!-- Accordion header -->
              <button (click)="selectItem(item)" class="w-full text-left p-4">
                <div class="flex items-center gap-2">
                  <svg class="w-4 h-4 text-text-secondary shrink-0 transition-transform duration-200"
                    [class.rotate-90]="item.id === selected()?.id"
                    fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" />
                  </svg>
                  <span class="text-sm font-medium text-text-primary">{{ item.title }}</span>
                  <span class="px-2 py-0.5 rounded-full text-xs font-medium {{ statusColor(item.status) }}">
                    {{ t('plans.status.' + item.status) }}
                  </span>
                </div>
                @if (item.id !== selected()?.id) {
                  @if (item.description) {
                    <p class="text-xs text-text-secondary line-clamp-1 mt-1 ml-6">{{ item.description }}</p>
                  }
                  <!-- Progress bar (collapsed) -->
                  @if (progressMap().get(item.id); as prog) {
                    <div class="mt-2 ml-6">
                      <div class="flex items-center justify-between text-xs text-text-secondary mb-1">
                        <span>{{ prog.done_tasks }}/{{ prog.total_tasks }} {{ t('plans.tasks') }}</span>
                        @if (prog.total_tasks > 0) {
                          <span>{{ progressPercent(prog) }}%</span>
                        }
                      </div>
                      <div class="w-full bg-surface-hover rounded-full h-1.5">
                        <div class="{{ progressColor(item.status) }} h-1.5 rounded-full transition-all"
                          [style.width.%]="progressPercent(prog)"></div>
                      </div>
                    </div>
                  }
                }
              </button>

              <!-- Expanded detail -->
              @if (item.id === selected()?.id) {
                <div class="px-4 pb-4 pt-0 border-t border-border/50 mt-0">
                  <div class="flex items-center gap-3 mb-4 flex-wrap pt-3">
                    <span class="px-2 py-0.5 rounded-full text-xs font-medium {{ statusColor(item.status) }}">
                      {{ t('plans.status.' + item.status) }}
                    </span>
                    @if (item.status === 'active') {
                      <button (click)="updateStatus(item, 'completed')"
                        class="px-3 py-1 text-xs font-medium bg-ctp-blue/20 text-ctp-blue rounded-lg hover:bg-ctp-blue/30">
                        {{ t('plans.markComplete') }}
                      </button>
                      <button (click)="updateStatus(item, 'cancelled')"
                        class="px-3 py-1 text-xs font-medium bg-ctp-overlay0/20 text-ctp-overlay0 rounded-lg hover:bg-ctp-overlay0/30">
                        {{ t('common.cancel') }}
                      </button>
                    } @else {
                      <button (click)="updateStatus(item, 'active')"
                        class="px-3 py-1 text-xs font-medium bg-ctp-green/20 text-ctp-green rounded-lg hover:bg-ctp-green/30">
                        {{ t('plans.reactivate') }}
                      </button>
                    }
                    <button (click)="openEdit(item)"
                      class="px-3 py-1 text-xs font-medium bg-accent/20 text-accent rounded-lg hover:bg-accent/30">
                      {{ t('plans.edit') }}
                    </button>
                    <button (click)="openDetail(item)"
                      class="px-3 py-1 text-xs font-medium bg-ctp-sapphire/20 text-ctp-sapphire rounded-lg hover:bg-ctp-sapphire/30">
                      {{ t('plans.viewTasks') }}
                    </button>
                    <button (click)="deletePlan(item)"
                      class="px-3 py-1 text-xs font-medium bg-ctp-red/20 text-ctp-red rounded-lg hover:bg-ctp-red/30 ml-auto">
                      {{ t('plans.delete') }}
                    </button>
                  </div>

                  @if (item.description) {
                    <p class="text-sm text-text-secondary mb-3 whitespace-pre-wrap">{{ item.description }}</p>
                  }

                  <!-- Progress -->
                  @if (progressMap().get(item.id); as prog) {
                    <div class="mb-3">
                      <div class="flex items-center justify-between text-xs text-text-secondary mb-1">
                        <span>{{ t('plans.progress') }}: {{ prog.done_tasks }}/{{ prog.total_tasks }} {{ t('plans.tasks') }}</span>
                        @if (prog.working_tasks > 0) {
                          <span class="text-ctp-yellow">{{ prog.working_tasks }} {{ t('plans.inProgress') }}</span>
                        }
                      </div>
                      <div class="w-full bg-surface-hover rounded-full h-2">
                        <div class="{{ progressColor(item.status) }} h-2 rounded-full transition-all"
                          [style.width.%]="progressPercent(prog)"></div>
                      </div>
                    </div>
                  }

                  <div class="text-xs text-text-secondary">
                    {{ t('plans.updatedAt') }}: {{ item.updated_at | date:'short' }}
                  </div>
                </div>
              }
            </div>
          }
        </div>
      }

      <!-- Create/Edit modal -->
      @if (showForm()) {
        <app-modal-wrapper (closed)="closeForm()">
          <h2 class="text-lg font-semibold text-text-primary mb-4">
            {{ editing() ? t('plans.editTitle') : t('plans.createTitle') }}
          </h2>
          <form (ngSubmit)="savePlan()" class="space-y-4">
            <div>
              <label for="planTitle" class="block text-sm font-medium text-text-primary mb-1">{{ t('plans.fieldTitle') }}</label>
              <input id="planTitle" type="text" [(ngModel)]="formTitle" name="title" required
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent" />
            </div>
            <div>
              <label for="planDescription" class="block text-sm font-medium text-text-primary mb-1">{{ t('plans.fieldDescription') }}</label>
              <textarea id="planDescription" [(ngModel)]="formDescription" name="description" rows="3"
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent"></textarea>
            </div>
            <div class="flex justify-end gap-3">
              <button type="button" (click)="closeForm()"
                class="px-4 py-2 text-sm text-text-secondary hover:text-text-primary">
                {{ t('plans.cancel') }}
              </button>
              <button type="submit"
                class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90">
                {{ t('plans.save') }}
              </button>
            </div>
          </form>
        </app-modal-wrapper>
      }
    </div>
  `,
})
export class PlanListPage extends CrudFeatureBase<SpPlan> {
  private plansApi = inject(PlansApiService);
  private router = inject(Router);

  readonly statuses = STATUSES;
  selectedStatus: PlanStatus | '' = '';

  // Progress data
  progressMap = signal<Map<string, SpPlanProgress>>(new Map());

  // Form fields
  formTitle = '';
  formDescription = '';

  // Computed
  filtered = computed(() => {
    const q = this.searchQuery().toLowerCase();
    return this.items().filter(p => !q || p.title.toLowerCase().includes(q) || (p.description ?? '').toLowerCase().includes(q));
  });

  loadItems(): void {
    this.loading.set(true);
    const status = this.selectedStatus || undefined;
    this.plansApi.list(status).subscribe({
      next: (res) => {
        this.items.set(res.data);
        this.loading.set(false);
        this.loadProgressForAll(res.data);
      },
      error: () => this.loading.set(false),
    });
  }

  private loadProgressForAll(plans: SpPlan[]): void {
    if (plans.length === 0) return;
    const requests = plans.map(p =>
      this.plansApi.progress(p.id).pipe(catchError(() => of(null))),
    );
    forkJoin(requests).subscribe(results => {
      const map = new Map<string, SpPlanProgress>();
      results.forEach((r) => {
        if (r) map.set(r.plan_id, r);
      });
      this.progressMap.set(map);
    });
  }

  statusColor(status: string): string {
    return STATUS_COLORS[status as PlanStatus] ?? 'bg-ctp-overlay0/20 text-ctp-overlay0';
  }

  progressColor(status: string): string {
    return PROGRESS_COLORS[status as PlanStatus] ?? 'bg-ctp-overlay0';
  }

  progressPercent(prog: SpPlanProgress): number {
    if (prog.total_tasks === 0) return 0;
    return Math.round((prog.done_tasks / prog.total_tasks) * 100);
  }

  updateStatus(plan: SpPlan, status: PlanStatus): void {
    this.plansApi.update(plan.id, { status }).subscribe(() => this.loadItems());
  }

  openDetail(plan: SpPlan): void {
    this.router.navigate(['/plans', plan.id]);
  }

  deletePlan(plan: SpPlan): void {
    this.plansApi.delete(plan.id).subscribe(() => {
      this.selected.set(null);
      this.loadItems();
    });
  }

  savePlan(): void {
    if (this.editing()) {
      this.plansApi.update(this.editing()!.id, {
        title: this.formTitle,
        description: this.formDescription || undefined,
      }).subscribe(() => {
        this.closeForm();
        this.loadItems();
      });
    } else {
      this.plansApi.create({
        title: this.formTitle,
        description: this.formDescription || undefined,
      }).subscribe(() => {
        this.closeForm();
        this.loadItems();
      });
    }
  }

  protected resetForm(): void {
    this.formTitle = '';
    this.formDescription = '';
  }

  protected fillForm(item: SpPlan): void {
    this.formTitle = item.title;
    this.formDescription = item.description ?? '';
  }
}
