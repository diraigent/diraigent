import { Component, inject, signal, computed } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { DatePipe } from '@angular/common';
import { LogsApiService, LogEntry } from '../../core/services/logs-api.service';

@Component({
  selector: 'app-logs',
  standalone: true,
  imports: [FormsModule, DatePipe],
  template: `
    <div class="p-3 sm:p-6 h-full flex flex-col">
      <!-- Header -->
      <div class="flex items-center justify-between mb-4">
        <h1 class="text-2xl font-semibold text-text-primary">Logs</h1>
        <div class="flex items-center gap-2 text-xs text-text-secondary">
          @if (lastFetched()) {
            <span>{{ totalEntries() }} entries · fetched {{ lastFetched() | date:'mediumTime' }}</span>
          }
        </div>
      </div>

      <!-- Query bar -->
      <div class="flex flex-wrap gap-3 mb-4">
        <input
          type="text"
          placeholder='LogQL query, e.g. {app="news-enrichment"}'
          [(ngModel)]="queryText"
          (keydown.enter)="search()"
          class="flex-1 min-w-[300px] bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary font-mono" />

        <!-- Label helper dropdown -->
        <select
          [(ngModel)]="selectedLabel"
          (ngModelChange)="onLabelSelect($event)"
          class="bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent">
          <option value="">Insert label…</option>
          @for (l of availableLabels(); track l) {
            <option [value]="l">{{ l }}</option>
          }
        </select>

        <!-- Time range -->
        <select
          [(ngModel)]="timeRange"
          class="bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent">
          <option value="15m">Last 15 min</option>
          <option value="1h">Last 1 hour</option>
          <option value="3h">Last 3 hours</option>
          <option value="6h">Last 6 hours</option>
          <option value="12h">Last 12 hours</option>
          <option value="24h">Last 24 hours</option>
          <option value="3d">Last 3 days</option>
          <option value="7d">Last 7 days</option>
        </select>

        <!-- Limit -->
        <select
          [(ngModel)]="limit"
          class="bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent">
          <option [ngValue]="50">50</option>
          <option [ngValue]="100">100</option>
          <option [ngValue]="200">200</option>
          <option [ngValue]="500">500</option>
          <option [ngValue]="1000">1000</option>
        </select>

        <button
          (click)="search()"
          [disabled]="loading() || !queryText.trim()"
          class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90
                 disabled:opacity-50 transition-opacity">
          {{ loading() ? 'Searching…' : 'Search' }}
        </button>
      </div>

      <!-- Filter bar -->
      <div class="flex gap-3 mb-3">
        <input
          type="text"
          placeholder="Filter log lines…"
          [(ngModel)]="filterText"
          class="flex-1 bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary" />
        <label class="flex items-center gap-1.5 text-sm text-text-secondary cursor-pointer">
          <input type="checkbox" [(ngModel)]="showLabels" class="rounded" />
          Labels
        </label>
        <label class="flex items-center gap-1.5 text-sm text-text-secondary cursor-pointer">
          <input type="checkbox" [(ngModel)]="wrapLines" class="rounded" />
          Wrap
        </label>
      </div>

      <!-- Error -->
      @if (error()) {
        <div class="mb-3 px-4 py-2 bg-ctp-red/10 border border-ctp-red/30 rounded-lg text-sm text-ctp-red">
          {{ error() }}
        </div>
      }

      <!-- Log output -->
      <div class="flex-1 min-h-0 overflow-auto bg-surface border border-border rounded-lg">
        @if (loading()) {
          <div class="flex items-center justify-center h-32">
            <div class="animate-spin w-6 h-6 border-2 border-accent border-t-transparent rounded-full"></div>
          </div>
        } @else if (filtered().length === 0) {
          <div class="flex items-center justify-center h-32 text-text-secondary text-sm">
            {{ entries().length === 0 ? 'No results. Enter a LogQL query and hit Search.' : 'No lines match filter.' }}
          </div>
        } @else {
          <table class="w-full text-xs font-mono">
            <tbody>
              @for (entry of filtered(); track $index) {
                <tr class="border-b border-border/30 align-top"
                    [class]="logRowClass(entry)">
                  <td class="px-3 py-1 whitespace-nowrap select-none w-[180px]"
                      [class.text-text-secondary]="logLevel(entry) === 'normal'">
                    {{ formatNanos(entry.timestamp) }}
                  </td>
                  @if (showLabels) {
                    <td class="px-2 py-1 w-[200px]"
                        [class.text-text-secondary]="logLevel(entry) === 'normal'">
                      <span class="truncate block max-w-[200px]" [title]="labelString(entry)">
                        {{ labelString(entry) }}
                      </span>
                    </td>
                  }
                  <td class="px-3 py-1"
                      [class.text-text-primary]="logLevel(entry) === 'normal'"
                      [class.whitespace-pre-wrap]="wrapLines"
                      [class.whitespace-nowrap]="!wrapLines">
                    {{ entry.line }}
                  </td>
                </tr>
              }
            </tbody>
          </table>
        }
      </div>
    </div>
  `,
})
export class LogsPage {
  private api = inject(LogsApiService);

  queryText = '{app=~".+"}';
  timeRange = '1h';
  limit = 100;
  filterText = '';
  showLabels = false;
  wrapLines = true;
  selectedLabel = '';

  loading = signal(false);
  error = signal<string | null>(null);
  entries = signal<LogEntry[]>([]);
  lastFetched = signal<Date | null>(null);
  totalEntries = signal(0);
  availableLabels = signal<string[]>([]);

  filtered = computed(() => {
    const f = this.filterText.toLowerCase().trim();
    const all = this.entries();
    if (!f) return all;
    return all.filter(e => e.line.toLowerCase().includes(f));
  });

  constructor() {
    this.loadLabels();
  }

  loadLabels(): void {
    this.api.labels().subscribe({
      next: (resp) => {
        if (Array.isArray(resp.data)) {
          this.availableLabels.set(resp.data.filter(l => l !== '__name__'));
        }
      },
    });
  }

  onLabelSelect(label: string): void {
    if (!label) return;
    // Load values for this label and insert into query
    this.api.labelValues(label).subscribe({
      next: (resp) => {
        if (Array.isArray(resp.data) && resp.data.length > 0) {
          const val = resp.data[0];
          const snippet = `{${label}="${val}"}`;
          if (!this.queryText.trim() || this.queryText.trim() === '{app=~".+"}') {
            this.queryText = snippet;
          }
        }
        this.selectedLabel = '';
      },
    });
  }

  search(): void {
    const query = this.queryText.trim();
    if (!query) return;

    this.loading.set(true);
    this.error.set(null);

    const now = new Date();
    const start = new Date(now.getTime() - this.parseRange(this.timeRange));

    this.api
      .query({
        query,
        start: start.toISOString(),
        end: now.toISOString(),
        limit: this.limit,
        direction: 'backward',
      })
      .subscribe({
        next: (resp) => {
          this.entries.set(resp.entries);
          this.totalEntries.set(resp.total);
          this.lastFetched.set(new Date());
          this.loading.set(false);
        },
        error: (err) => {
          this.error.set(err?.error?.message || err?.message || 'Request failed');
          this.loading.set(false);
        },
      });
  }

  formatNanos(nanos: string): string {
    try {
      const ms = Number(BigInt(nanos) / BigInt(1_000_000));
      return new Date(ms).toISOString().replace('T', ' ').replace('Z', '');
    } catch {
      return nanos;
    }
  }

  labelString(entry: LogEntry): string {
    return Object.entries(entry.labels)
      .map(([k, v]) => `${k}=${v}`)
      .join(', ');
  }

  logLevel(entry: LogEntry): 'error' | 'warn' | 'normal' {
    const level = (entry.labels['level'] || entry.labels['severity'] || '').toUpperCase();
    if (level === 'ERROR' || /\bERROR\b/.test(entry.line)) return 'error';
    if (level === 'WARN' || level === 'WARNING' || /\bWARN(?:ING)?\b/.test(entry.line)) return 'warn';
    return 'normal';
  }

  logRowClass(entry: LogEntry): string {
    switch (this.logLevel(entry)) {
      case 'error': return 'bg-ctp-red/10 text-ctp-red hover:bg-ctp-red/15';
      case 'warn':  return 'bg-ctp-yellow/10 text-ctp-yellow hover:bg-ctp-yellow/15';
      default:      return 'hover:bg-accent/5';
    }
  }

  private parseRange(range: string): number {
    const match = range.match(/^(\d+)([mhd])$/);
    if (!match) return 3600_000;
    const val = parseInt(match[1], 10);
    switch (match[2]) {
      case 'm': return val * 60_000;
      case 'h': return val * 3600_000;
      case 'd': return val * 86400_000;
      default: return 3600_000;
    }
  }
}
