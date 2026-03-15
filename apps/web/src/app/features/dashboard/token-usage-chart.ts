import { Component, inject, input, signal, computed, effect, DestroyRef } from '@angular/core';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { BaseChartDirective } from 'ng2-charts';
import { ChartConfiguration, ChartData } from 'chart.js';
import {
  Chart,
  LineController,
  LineElement,
  PointElement,
  LinearScale,
  LogarithmicScale,
  CategoryScale,
  Filler,
  Tooltip,
  Legend,
} from 'chart.js';
import { forkJoin, of } from 'rxjs';
import { catchError, map } from 'rxjs/operators';
import { DiraigentApiService, TokenDayCount } from '../../core/services/diraigent-api.service';

Chart.register(LineController, LineElement, PointElement, LinearScale, LogarithmicScale, CategoryScale, Filler, Tooltip, Legend);

type TimeRange = 7 | 30 | 90;

export interface ChartProject {
  id: string;
  name: string;
}

interface ProjectTokenData {
  project: ChartProject;
  tokensPerDay: TokenDayCount[];
}

const CHART_COLORS = [
  { border: '#89b4fa', bg: 'rgba(137, 180, 250, 0.15)' },  // blue
  { border: '#a6e3a1', bg: 'rgba(166, 227, 161, 0.15)' },  // green
  { border: '#f38ba8', bg: 'rgba(243, 139, 168, 0.15)' },  // red
  { border: '#fab387', bg: 'rgba(250, 179, 135, 0.15)' },  // peach
  { border: '#cba6f7', bg: 'rgba(203, 166, 247, 0.15)' },  // mauve
  { border: '#f9e2af', bg: 'rgba(249, 226, 175, 0.15)' },  // yellow
  { border: '#94e2d5', bg: 'rgba(148, 226, 213, 0.15)' },  // teal
  { border: '#f5c2e7', bg: 'rgba(245, 194, 231, 0.15)' },  // pink
  { border: '#74c7ec', bg: 'rgba(116, 199, 236, 0.15)' },  // sapphire
  { border: '#b4befe', bg: 'rgba(180, 190, 254, 0.15)' },  // lavender
];

@Component({
  selector: 'app-token-usage-chart',
  standalone: true,
  imports: [BaseChartDirective],
  template: `
    <div class="bg-surface border border-border rounded-lg p-4">
      <div class="flex items-center justify-between mb-4">
        <h3 class="text-sm font-medium text-text-primary">Token Usage Over Time</h3>
        <div class="flex gap-1">
          @for (range of timeRanges; track range) {
            <button
              (click)="setRange(range)"
              class="px-2.5 py-1 text-xs rounded-md transition-colors cursor-pointer"
              [class]="selectedRange() === range
                ? 'bg-ctp-lavender/20 text-ctp-lavender font-medium'
                : 'text-text-muted hover:text-text-secondary hover:bg-surface-hover'">
              {{ range }}d
            </button>
          }
        </div>
      </div>

      @if (loading()) {
        <div class="flex items-center justify-center h-48">
          <span class="text-sm text-text-muted">Loading chart data…</span>
        </div>
      } @else if (isEmpty()) {
        <div class="flex items-center justify-center h-48">
          <span class="text-sm text-text-muted">No token usage data for this period</span>
        </div>
      } @else {
        <div class="h-56">
          <canvas baseChart
            [data]="chartData()"
            [options]="chartOptions"
            [type]="'line'">
          </canvas>
        </div>
      }
    </div>
  `,
})
export class TokenUsageChartComponent {
  private api = inject(DiraigentApiService);
  private destroyRef = inject(DestroyRef);

  /** Projects to display in the chart */
  projects = input.required<ChartProject[]>();

  readonly timeRanges: TimeRange[] = [7, 30, 90];
  selectedRange = signal<TimeRange>(30);
  loading = signal(false);

  /** Aggregated token data per project */
  private projectData = signal<ProjectTokenData[]>([]);

  isEmpty = computed(() => {
    const data = this.projectData();
    return data.length === 0 || data.every(pd =>
      pd.tokensPerDay.length === 0 ||
      pd.tokensPerDay.every(d => d.input_tokens === 0 && d.output_tokens === 0),
    );
  });

  chartData = computed<ChartData<'line'>>(() => {
    const allData = this.projectData();
    if (allData.length === 0) return { labels: [], datasets: [] };

    // Collect all unique days across all projects, sorted chronologically
    const daySet = new Set<string>();
    for (const pd of allData) {
      for (const d of pd.tokensPerDay) {
        daySet.add(d.day);
      }
    }
    const allDays = [...daySet].sort();

    // One dataset per project showing total tokens (input + output)
    const datasets = allData.map((pd, i) => {
      const color = CHART_COLORS[i % CHART_COLORS.length];
      const dayMap = new Map<string, { input: number; output: number }>();
      for (const d of pd.tokensPerDay) {
        dayMap.set(d.day, { input: d.input_tokens ?? 0, output: d.output_tokens ?? 0 });
      }

      return {
        label: pd.project.name,
        data: allDays.map(day => {
          const entry = dayMap.get(day);
          if (!entry) return null;
          const total = entry.input + entry.output;
          return total > 0 ? total : null; // null for zero to avoid log(0)
        }),
        borderColor: color.border,
        backgroundColor: color.bg,
        fill: true,
        tension: 0.3,
        pointRadius: 2,
        pointHoverRadius: 5,
        borderWidth: 2,
        spanGaps: true,
      };
    });

    return {
      labels: allDays.map(d => this.formatLabel(d)),
      datasets,
    };
  });

  /** Lookup for tooltip: per-project per-day input/output breakdown */
  private dayBreakdown = computed(() => {
    const allData = this.projectData();
    const result = new Map<string, Map<string, { input: number; output: number }>>();
    for (const pd of allData) {
      const dayMap = new Map<string, { input: number; output: number }>();
      for (const d of pd.tokensPerDay) {
        dayMap.set(d.day, { input: d.input_tokens ?? 0, output: d.output_tokens ?? 0 });
      }
      result.set(pd.project.name, dayMap);
    }
    return result;
  });

  chartOptions: ChartConfiguration<'line'>['options'] = {
    responsive: true,
    maintainAspectRatio: false,
    interaction: {
      mode: 'index',
      intersect: false,
    },
    plugins: {
      legend: {
        display: true,
        position: 'bottom',
        labels: {
          color: '#a6adc8', // ctp-subtext0
          boxWidth: 12,
          padding: 16,
          font: { size: 11 },
        },
      },
      tooltip: {
        backgroundColor: '#313244', // ctp-surface0
        titleColor: '#cdd6f4',      // ctp-text
        bodyColor: '#bac2de',       // ctp-subtext1
        borderColor: '#45475a',     // ctp-surface1
        borderWidth: 1,
        padding: 10,
        callbacks: {
          label: (ctx) => {
            const val = ctx.parsed.y ?? 0;
            const formatted = this.formatTokenValue(val);
            const projectName = ctx.dataset.label ?? '';

            // Look up input/output breakdown
            const breakdown = this.dayBreakdown();
            const projectDays = breakdown.get(projectName);
            const allDays = this.getAllDays();
            const dayKey = allDays[ctx.dataIndex];
            const entry = dayKey ? projectDays?.get(dayKey) : undefined;

            if (entry) {
              return `${projectName}: ${formatted} (${this.formatTokenValue(entry.input)} in / ${this.formatTokenValue(entry.output)} out)`;
            }
            return `${projectName}: ${formatted}`;
          },
        },
      },
    },
    scales: {
      x: {
        grid: {
          color: 'rgba(69, 71, 90, 0.3)', // ctp-surface1 with alpha
        },
        ticks: {
          color: '#6c7086', // ctp-overlay0
          font: { size: 10 },
          maxRotation: 0,
          autoSkip: true,
          maxTicksLimit: 10,
        },
      },
      y: {
        type: 'logarithmic',
        min: 1,
        grid: {
          color: 'rgba(69, 71, 90, 0.3)',
        },
        ticks: {
          color: '#6c7086',
          font: { size: 10 },
          callback: function(value) {
            const num = Number(value ?? 0);
            // Only show ticks at powers of 10 to avoid clutter on log scale
            if (num <= 0) return '';
            const log = Math.log10(num);
            if (Math.abs(log - Math.round(log)) > 0.01) return '';
            if (num >= 1_000_000) return (num / 1_000_000).toFixed(0) + 'M';
            if (num >= 1_000) return (num / 1_000).toFixed(0) + 'K';
            return num.toString();
          },
        },
      },
    },
  };

  constructor() {
    // Fetch data on init and whenever projects or selectedRange changes
    effect(() => {
      const projects = this.projects();
      const days = this.selectedRange();
      this.fetchAllMetrics(projects, days);
    });
  }

  setRange(range: TimeRange): void {
    this.selectedRange.set(range);
  }

  private fetchAllMetrics(projects: ChartProject[], days: number): void {
    if (projects.length === 0) {
      this.projectData.set([]);
      return;
    }

    this.loading.set(true);

    forkJoin(
      projects.map(project =>
        this.api.getProjectMetrics(project.id, days).pipe(
          map(metrics => ({
            project,
            tokensPerDay: metrics.tokens_per_day ?? [],
          } as ProjectTokenData)),
          catchError(() => of({
            project,
            tokensPerDay: [],
          } as ProjectTokenData)),
        ),
      ),
    )
      .pipe(takeUntilDestroyed(this.destroyRef))
      .subscribe({
        next: (data) => {
          this.projectData.set(data);
          this.loading.set(false);
        },
        error: () => {
          this.projectData.set([]);
          this.loading.set(false);
        },
      });
  }

  /** Get sorted unique days across all projects (needed for tooltip day lookup) */
  private getAllDays(): string[] {
    const daySet = new Set<string>();
    for (const pd of this.projectData()) {
      for (const d of pd.tokensPerDay) {
        daySet.add(d.day);
      }
    }
    return [...daySet].sort();
  }

  private formatLabel(day: string): string {
    const date = new Date(day + 'T00:00:00');
    return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
  }

  private formatTokenValue(val: number): string {
    if (val >= 1_000_000) return (val / 1_000_000).toFixed(1) + 'M';
    if (val >= 1_000) return (val / 1_000).toFixed(1) + 'K';
    return val.toString();
  }
}
