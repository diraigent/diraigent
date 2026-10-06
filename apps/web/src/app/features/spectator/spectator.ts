import { Component, inject, signal } from '@angular/core';
import { ActivatedRoute, RouterLink } from '@angular/router';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { TranslocoPipe } from '@jsverse/transloco';
import { catchError, combineLatest, forkJoin, map, of, switchMap } from 'rxjs';
import {
  PublicDetail, PublicItem, PublicPage, PublicProject, SpectatorApiService, SpectatorArea,
} from '../../core/services/spectator-api.service';

@Component({
  selector: 'app-spectator',
  standalone: true,
  imports: [RouterLink, TranslocoPipe],
  template: `
    <div class="max-w-5xl mx-auto p-6 space-y-6">
      <header class="flex flex-wrap items-center justify-between gap-4 border-b border-border pb-4">
        <p class="font-semibold text-accent">{{ 'spectator.banner' | transloco }}</p>
        <!-- Full navigation deliberately restores normal OAuth/bootstrap outside this shell. -->
        <a href="/dashboard" class="underline">{{ 'spectator.exit' | transloco }}</a>
      </header>
      @if (loading()) {
        <p role="status">{{ 'spectator.loading' | transloco }}</p>
      } @else if (unavailable()) {
        <section role="alert" class="space-y-4">
          <h1 class="text-2xl font-semibold">{{ 'spectator.unavailable' | transloco }}</h1>
          <p>{{ 'spectator.unavailableHint' | transloco }}</p>
          <button class="underline" (click)="reload()">{{ 'spectator.retry' | transloco }}</button>
        </section>
      } @else if (project(); as p) {
        <section class="space-y-3">
          <h1 class="text-3xl font-semibold break-words">{{ p.name }}</h1>
          <p class="whitespace-pre-wrap break-words">{{ p.description }}</p>
        </section>
        <nav class="flex flex-wrap gap-4" [attr.aria-label]="'spectator.sections' | transloco">
          @for (tab of areas; track tab) {
            <a [routerLink]="[]" [queryParams]="{ area: tab }" class="underline"
               [attr.aria-current]="area() === tab ? 'page' : null">{{ ('spectator.' + tab) | transloco }}</a>
          }
        </nav>
        @if (detail(); as item) {
          <article class="space-y-4 border border-border rounded-lg p-4">
            <a [routerLink]="[]" [queryParams]="{ area: area(), offset: offset() }" class="underline">
              {{ 'spectator.back' | transloco }}
            </a>
            <h2 class="text-2xl font-semibold break-words">{{ item.title }}</h2>
            <p class="text-text-secondary">{{ status(item) }}</p>
            @for (section of sections(item); track section.label) {
              <section class="space-y-2">
                <h3 class="font-semibold">{{ ('spectator.' + section.label) | transloco }}</h3>
                <!-- Interpolation renders published text, never executable HTML. -->
                @for (text of section.texts; track $index) {
                  <p class="whitespace-pre-wrap break-words">{{ text }}</p>
                }
              </section>
            }
          </article>
        } @else if (page(); as result) {
          <h2 class="text-xl font-semibold">{{ ('spectator.' + area()) | transloco }}</h2>
          @if (!result.data.length) {
            <p role="status">{{ 'spectator.empty' | transloco }}</p>
          }
          <ul class="space-y-2">
            @for (item of result.data; track item.id) {
              <li class="border border-border rounded-lg p-4">
                <a [routerLink]="[]" [queryParams]="{ area: area(), offset: offset(), id: item.id }"
                   class="underline break-words">{{ item.title }}</a>
                <p class="text-text-secondary">{{ status(item) }}</p>
              </li>
            }
          </ul>
          <nav class="flex items-center gap-4" [attr.aria-label]="'spectator.pagination' | transloco">
            @if (offset() > 0) {
              <a [routerLink]="[]" [queryParams]="{ area: area(), offset: previousOffset() }" class="underline">
                {{ 'spectator.previous' | transloco }}
              </a>
            }
            <span>{{ 'spectator.page' | transloco }} {{ pageNumber() }}</span>
            @if (result.has_more && offset() < maxOffset) {
              <a [routerLink]="[]" [queryParams]="{ area: area(), offset: offset() + pageSize }" class="underline">
                {{ 'spectator.next' | transloco }}
              </a>
            }
          </nav>
        }
      }
    </div>
  `,
})
export class SpectatorPage {
  private route = inject(ActivatedRoute);
  private api = inject(SpectatorApiService);
  readonly areas: SpectatorArea[] = ['tasks', 'work', 'knowledge', 'decisions'];
  readonly pageSize = this.api.pageSize;
  readonly maxOffset = 1_000_000;
  readonly project = signal<PublicProject | null>(null);
  readonly page = signal<PublicPage | null>(null);
  readonly detail = signal<PublicDetail | null>(null);
  readonly area = signal<SpectatorArea>('tasks');
  readonly offset = signal(0);
  readonly loading = signal(true);
  readonly unavailable = signal(false);

  constructor() {
    combineLatest([this.route.paramMap, this.route.queryParamMap]).pipe(
      switchMap(([params, query]) => {
        this.project.set(null);
        this.page.set(null);
        this.detail.set(null);
        this.loading.set(true);
        this.unavailable.set(false);
        const requestedArea = query.get('area');
        const area = this.areas.find(a => a === requestedArea) ?? 'tasks';
        const requestedOffset = Number(query.get('offset'));
        // Bound deep-link offsets and always request fixed-size pages.
        const offset = Number.isSafeInteger(requestedOffset) && requestedOffset >= 0
          ? Math.min(this.maxOffset, Math.floor(requestedOffset / this.pageSize) * this.pageSize) : 0;
        this.area.set(area);
        this.offset.set(offset);
        const projectId = params.get('projectId')!;
        const id = query.get('id');
        return forkJoin({
          project: this.api.project(projectId),
          content: id ? this.api.detail(projectId, area, id) : this.api.list(projectId, area, offset),
        }).pipe(
          map(result => ({ ...result, isDetail: !!id })),
          catchError(() => of(null)),
        );
      }),
      takeUntilDestroyed(),
    ).subscribe(result => {
      this.loading.set(false);
      if (!result || result.project.spectator !== true) {
        this.unavailable.set(true);
        return;
      }
      this.project.set(result.project);
      if (result.isDetail) this.detail.set(result.content as PublicDetail);
      else this.page.set(result.content as PublicPage);
    });
  }

  reload(): void { window.location.reload(); }
  previousOffset(): number { return Math.max(0, this.offset() - this.pageSize); }
  pageNumber(): number { return Math.floor(this.offset() / this.pageSize) + 1; }

  status(item: PublicItem): string {
    if ('state' in item) return `#${item.number} · ${item.state} · ${item.kind}`;
    if ('category' in item) return item.category;
    return item.status;
  }

  sections(item: PublicDetail): { label: string; texts: string[] }[] {
    if ('spec' in item) return [
      { label: 'spec', texts: item.spec ? [item.spec] : [] },
      { label: 'criteria', texts: item.acceptance_criteria },
    ];
    if ('work_type' in item) return [
      { label: 'description', texts: item.description ? [item.description] : [] },
      { label: 'criteria', texts: item.success_criteria },
    ];
    if ('content' in item) return [
      { label: 'content', texts: [item.content] },
      { label: 'tags', texts: item.tags },
    ];
    return [
      { label: 'context', texts: [item.context] },
      { label: 'decision', texts: item.decision ? [item.decision] : [] },
      { label: 'rationale', texts: item.rationale ? [item.rationale] : [] },
    ];
  }
}
