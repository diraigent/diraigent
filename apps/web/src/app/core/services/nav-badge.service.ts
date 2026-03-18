import { Injectable, inject, signal } from '@angular/core';
import { toObservable } from '@angular/core/rxjs-interop';
import { switchMap, catchError, of, timer } from 'rxjs';
import { forkJoin, map } from 'rxjs';
import { ObservationsApiService } from './observations-api.service';
import { DecisionsApiService } from './decisions-api.service';
import { TasksApiService } from './tasks-api.service';
import { DiraigentApiService } from './diraigent-api.service';
import { ProjectContext } from './project-context.service';
import { ReviewSseService } from './review-sse.service';

/** How often (ms) to re-fetch badge counts while a project is active. */
const POLL_INTERVAL_MS = 300_000;

@Injectable({ providedIn: 'root' })
export class NavBadgeService {
  private obsApi = inject(ObservationsApiService);
  private decisionsApi = inject(DecisionsApiService);
  private tasksApi = inject(TasksApiService);
  private diraigentApi = inject(DiraigentApiService);
  private ctx = inject(ProjectContext);
  private reviewSse = inject(ReviewSseService);

  /** Count of open (unresolved) observations */
  readonly openObservations = signal(0);

  /** Count of proposed (pending) decisions */
  readonly proposedDecisions = signal(0);

  /** Count of tasks awaiting human review (across all projects) */
  readonly humanReviewCount = signal(0);

  constructor() {
    toObservable(this.ctx.projectId)
      .pipe(
        switchMap(id => {
          if (!id) return of({ obs: 0, dec: 0 });
          // Emit immediately (delay=0) then repeat every POLL_INTERVAL_MS.
          // The outer switchMap cancels the timer when the project changes.
          return timer(0, POLL_INTERVAL_MS).pipe(
            switchMap(() =>
              forkJoin({
                obs: this.obsApi.list('open').pipe(
                  map(items => items.length),
                  catchError(() => of(0)),
                ),
                dec: this.decisionsApi.list('proposed').pipe(
                  map(items => items.length),
                  catchError(() => of(0)),
                ),
              }),
            ),
          );
        }),
      )
      .subscribe(({ obs, dec }) => {
        this.openObservations.set(obs);
        this.proposedDecisions.set(dec);
      });

    // Fetch review count on startup, then re-fetch whenever the SSE stream
    // signals a human_review state change (replaces 30 s polling timer).
    const fetchReviewCount$ = this.diraigentApi.getProjects().pipe(
      catchError(() => of([])),
      switchMap(projects => {
        if (!projects.length) return of(0);
        return forkJoin(
          projects.map(p =>
            this.tasksApi.listForProject(p.id, { state: 'human_review', limit: 100 }).pipe(
              map(resp => resp.data.length),
              catchError(() => of(0)),
            ),
          ),
        ).pipe(map(counts => counts.reduce((a, b) => a + b, 0)));
      }),
    );

    // Initial load
    fetchReviewCount$.subscribe(count => this.humanReviewCount.set(count));

    // SSE-triggered updates
    this.reviewSse.events
      .pipe(switchMap(() => fetchReviewCount$))
      .subscribe(count => this.humanReviewCount.set(count));
  }
}
