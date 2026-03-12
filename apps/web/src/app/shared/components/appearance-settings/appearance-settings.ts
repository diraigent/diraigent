import { Component, inject } from '@angular/core';
import { TranslocoModule } from '@jsverse/transloco';
import { ThemeService, ThemePreference, AccentColor, ACCENT_COLORS } from '../../../core/services/theme.service';

interface FlavorOption {
  value: ThemePreference;
  label: string;
  description: string;
}

const FLAVOR_OPTIONS: FlavorOption[] = [
  { value: 'catppuccin-latte',     label: 'Latte',     description: 'Light, warm'   },
  { value: 'catppuccin-frappe',    label: 'Frappé',    description: 'Dark, cool'    },
  { value: 'catppuccin-macchiato', label: 'Macchiato', description: 'Dark, deeper'  },
  { value: 'catppuccin-mocha',     label: 'Mocha',     description: 'Dark, richest' },
];

/** Representative palette dots per flavor (base, surface, accent, blue). */
const FLAVOR_DOTS: Record<string, string[]> = {
  'catppuccin-latte':     ['#eff1f5', '#ccd0da', '#8839ef', '#1e66f5'],
  'catppuccin-frappe':    ['#303446', '#414559', '#ca9ee6', '#8caaee'],
  'catppuccin-macchiato': ['#24273a', '#363a4f', '#c6a0f6', '#8aadf4'],
  'catppuccin-mocha':     ['#1e1e2e', '#313244', '#cba6f7', '#89b4fa'],
};

/** Hex colors per accent × all 4 flavors (using mocha as representative). */
const ACCENT_HEX: Record<AccentColor, string> = {
  rosewater: '#f5e0dc',
  flamingo:  '#f2cdcd',
  pink:      '#f5c2e7',
  mauve:     '#cba6f7',
  red:       '#f38ba8',
  maroon:    '#eba0ac',
  peach:     '#fab387',
  yellow:    '#f9e2af',
  green:     '#a6e3a1',
  teal:      '#94e2d5',
  sky:       '#89dceb',
  sapphire:  '#74c7ec',
  blue:      '#89b4fa',
  lavender:  '#b4befe',
};

const ACCENT_LABELS: Record<AccentColor, string> = {
  rosewater: 'Rosewater',
  flamingo:  'Flamingo',
  pink:      'Pink',
  mauve:     'Mauve',
  red:       'Red',
  maroon:    'Maroon',
  peach:     'Peach',
  yellow:    'Yellow',
  green:     'Green',
  teal:      'Teal',
  sky:       'Sky',
  sapphire:  'Sapphire',
  blue:      'Blue',
  lavender:  'Lavender',
};

@Component({
  selector: 'app-appearance-settings',
  standalone: true,
  imports: [TranslocoModule],
  template: `
    <div class="space-y-6" *transloco="let t">

      <!-- Flavor (color mode) -->
      <div>
        <p class="text-sm font-medium text-text-secondary mb-3">{{ t('tenantSettings.colorMode') }}</p>
        <div class="grid grid-cols-2 gap-3 sm:grid-cols-4">
          @for (flavor of flavors; track flavor.value) {
            <button
              (click)="theme.setTheme(flavor.value)"
              class="flex flex-col items-center gap-1 px-3 py-3 rounded-lg border-2 text-text-primary transition-colors"
              [class.border-accent]="theme.preference() === flavor.value"
              [class.bg-surface]="theme.preference() === flavor.value"
              [class.border-border]="theme.preference() !== flavor.value"
              [class.bg-bg-subtle]="theme.preference() !== flavor.value"
              [title]="flavor.label + ' — ' + flavor.description"
            >
              <span class="flex gap-0.5 mb-2 justify-center">
                @for (dot of flavorDots(flavor.value); track dot) {
                  <span class="w-3 h-3 rounded-full" [style.background]="dot"></span>
                }
              </span>
              <span class="text-sm font-medium">{{ flavor.label }}</span>
              <span class="text-xs opacity-70">{{ flavor.description }}</span>
            </button>
          }
        </div>

        <!-- System preference option -->
        <button
          (click)="theme.setTheme('system')"
          class="mt-2 w-full flex items-center gap-2 px-3 py-2 rounded-lg text-sm border transition-colors"
          [class.border-accent]="theme.preference() === 'system'"
          [class.text-text-primary]="theme.preference() === 'system'"
          [class.border-border]="theme.preference() !== 'system'"
          [class.text-text-secondary]="theme.preference() !== 'system'"
        >
          <svg class="w-4 h-4 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5"
              d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"/>
          </svg>
          {{ t('tenantSettings.followSystem') }}
          @if (theme.preference() === 'system') {
            <span class="ml-auto text-xs px-1.5 py-0.5 rounded bg-accent text-bg">{{ t('tenantSettings.systemActive') }}</span>
          }
        </button>
      </div>

      <!-- Accent color -->
      <div>
        <p class="text-sm font-medium text-text-secondary mb-3">{{ t('tenantSettings.accentColor') }}</p>
        <div class="flex flex-wrap gap-2">
          @for (acc of accents; track acc) {
            <button
              (click)="theme.setAccent(acc)"
              [title]="accentLabel(acc)"
              [style.background]="accentHex(acc)"
              class="w-8 h-8 rounded-full border-2 transition-transform hover:scale-110 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-offset-bg"
              [class.border-text-primary]="theme.accent() === acc"
              [class.scale-110]="theme.accent() === acc"
              [class.border-transparent]="theme.accent() !== acc"
            >
              <span class="sr-only">{{ accentLabel(acc) }}</span>
            </button>
          }
        </div>
        <p class="mt-2 text-xs text-text-muted">
          {{ t('tenantSettings.currentAccent') }} <span class="text-text-secondary font-medium">{{ accentLabel(theme.accent()) }}</span>
        </p>
      </div>

    </div>
  `,
})
export class AppearanceSettingsComponent {
  theme = inject(ThemeService);

  readonly flavors = FLAVOR_OPTIONS;
  readonly accents = ACCENT_COLORS;

  accentLabel(acc: AccentColor): string {
    return ACCENT_LABELS[acc];
  }

  accentHex(acc: AccentColor): string {
    return ACCENT_HEX[acc];
  }

  flavorDots(flavor: ThemePreference): string[] {
    return FLAVOR_DOTS[flavor] ?? FLAVOR_DOTS['catppuccin-mocha'];
  }
}
