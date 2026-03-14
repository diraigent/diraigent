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
import { DiraigentApiService, TokenDayCount } from '../../core/services/diraigent-api.service';

Chart.register(LineController, LineElement, PointElement, LinearScale, LogarithmicScale, CategoryScale, Filler, Tooltip, Legend);

type TimeRange = 7 | 30 | 90;

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

  /** Project ID for this chart */
  projectId = input.required<string>();

  readonly timeRanges: TimeRange[] = [7, 30, 90];
  selectedRange = signal<TimeRange>(30);
  loading = signal(false);

  /** Raw tokens_per_day from the metrics API */
  private tokensPerDay = signal<TokenDayCount[]>([]);

  isEmpty = computed(() => {
    const data = this.tokensPerDay();
    return data.length === 0 || data.every(d => d.input_tokens === 0 && d.output_tokens === 0);
  });

  chartData = computed<ChartData<'line'>>(() => {
    const data = this.tokensPerDay();
    return {
      labels: data.map(d => this.formatLabel(d.day)),
      datasets: [
        {
          label: 'Input Tokens',
          data: data.map(d => d.input_tokens || null),  // null for zero to avoid log(0)
          borderColor: '#89b4fa',  // ctp-blue
          backgroundColor: 'rgba(137, 180, 250, 0.1)',
          fill: true,
          tension: 0.3,
          pointRadius: 2,
          pointHoverRadius: 5,
          borderWidth: 2,
          spanGaps: true,
        },
        {
          label: 'Output Tokens',
          data: data.map(d => d.output_tokens || null),  // null for zero to avoid log(0)
          borderColor: '#a6e3a1',  // ctp-green
          backgroundColor: 'rgba(166, 227, 161, 0.1)',
          fill: true,
          tension: 0.3,
          pointRadius: 2,
          pointHoverRadius: 5,
          borderWidth: 2,
          spanGaps: true,
        },
      ],
    };
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
          label: function(ctx) {
            const val = ctx.parsed.y ?? 0;
            const formatted = val >= 1_000_000
              ? (val / 1_000_000).toFixed(1) + 'M'
              : val >= 1_000
                ? (val / 1_000).toFixed(1) + 'K'
                : val.toString();
            return `${ctx.dataset.label ?? ''}: ${formatted}`;
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
    // Fetch data on init and whenever selectedRange changes
    effect(() => {
      const projectId = this.projectId();
      const days = this.selectedRange();
      this.fetchMetrics(projectId, days);
    });
  }

  setRange(range: TimeRange): void {
    this.selectedRange.set(range);
  }

  private fetchMetrics(projectId: string, days: number): void {
    this.loading.set(true);
    this.api.getProjectMetrics(projectId, days)
      .pipe(takeUntilDestroyed(this.destroyRef))
      .subscribe({
        next: (metrics) => {
          this.tokensPerDay.set(metrics.tokens_per_day ?? []);
          this.loading.set(false);
        },
        error: () => {
          this.tokensPerDay.set([]);
          this.loading.set(false);
        },
      });
  }

  private formatLabel(day: string): string {
    const date = new Date(day + 'T00:00:00');
    return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
  }
}
