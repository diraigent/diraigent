import { Injectable, inject, signal, computed } from '@angular/core';
import { PLATFORM_ID } from '@angular/core';
import { DOCUMENT, isPlatformBrowser } from '@angular/common';
import { TenantApiService } from './tenant-api.service';

export type ThemePreference = 'system' | 'catppuccin-latte' | 'catppuccin-frappe' | 'catppuccin-macchiato' | 'catppuccin-mocha';
export type AccentColor = 'rosewater' | 'flamingo' | 'pink' | 'mauve' | 'red' | 'maroon' | 'peach' | 'yellow' | 'green' | 'teal' | 'sky' | 'sapphire' | 'blue' | 'lavender';

export const CATPPUCCIN_FLAVORS = ['latte', 'frappe', 'macchiato', 'mocha'] as const;
export const ACCENT_COLORS: AccentColor[] = [
  'rosewater', 'flamingo', 'pink', 'mauve', 'red', 'maroon', 'peach',
  'yellow', 'green', 'teal', 'sky', 'sapphire', 'blue', 'lavender',
];

const DARK_THEMES = new Set<string>(['catppuccin-frappe', 'catppuccin-macchiato', 'catppuccin-mocha']);

const VALID_THEMES = new Set<string>([
  'system', 'catppuccin-latte', 'catppuccin-frappe', 'catppuccin-macchiato', 'catppuccin-mocha',
]);
const VALID_ACCENTS = new Set<string>(ACCENT_COLORS);

const themeKey = (tenantId?: string | null) => tenantId ? `zivue-theme-${tenantId}` : 'zivue-theme';
const accentKey = (tenantId?: string | null) => tenantId ? `zivue-accent-${tenantId}` : 'zivue-accent';

@Injectable({ providedIn: 'root' })
export class ThemeService {
  private platformId = inject(PLATFORM_ID);
  private document = inject(DOCUMENT);
  private tenantApi = inject(TenantApiService);

  /** Currently active tenant ID (set by settings page after tenant loads). */
  readonly tenantId = signal<string | null>(null);

  readonly preference = signal<ThemePreference>('system');
  readonly accent = signal<AccentColor>('blue');

  readonly resolvedTheme = computed(() => this.resolve(this.preference()));
  readonly isDark = computed(() => DARK_THEMES.has(this.resolvedTheme()));

  constructor() {
    if (isPlatformBrowser(this.platformId)) {
      const stored = (localStorage.getItem(themeKey()) || 'system') as ThemePreference;
      const storedAccent = (localStorage.getItem(accentKey()) || 'blue') as AccentColor;
      this.preference.set(stored);
      this.accent.set(storedAccent);
      this.applyTheme(this.resolve(stored));
      this.applyAccent(storedAccent);

      window.matchMedia('(prefers-color-scheme: dark)')
        .addEventListener('change', () => {
          if (this.preference() === 'system') {
            this.applyTheme(this.resolve('system'));
          }
        });
    }
  }

  /**
   * Called when a tenant is identified (e.g. on settings page load).
   * If server-side preferences are provided, they take priority over localStorage.
   * Falls back to localStorage if no server preferences are given.
   */
  setTenant(tenantId: string | null, serverTheme?: string | null, serverAccent?: string | null): void {
    this.tenantId.set(tenantId);
    if (!isPlatformBrowser(this.platformId)) return;

    // Server preferences take priority when valid; otherwise fall back to localStorage
    const localTheme = localStorage.getItem(themeKey(tenantId)) || 'system';
    const localAccent = localStorage.getItem(accentKey(tenantId)) || 'blue';

    const theme = (serverTheme && VALID_THEMES.has(serverTheme) ? serverTheme : localTheme) as ThemePreference;
    const accent = (serverAccent && VALID_ACCENTS.has(serverAccent) ? serverAccent : localAccent) as AccentColor;

    this.preference.set(theme);
    this.accent.set(accent);
    this.applyTheme(this.resolve(theme));
    this.applyAccent(accent);

    // Sync server values into localStorage for FOUC prevention
    if (tenantId) {
      localStorage.setItem(themeKey(tenantId), theme);
      localStorage.setItem(accentKey(tenantId), accent);
      localStorage.setItem('zivue-theme', theme);
      localStorage.setItem('zivue-accent', accent);
    }
  }

  toggle(): void {
    const next: ThemePreference = this.isDark() ? 'catppuccin-latte' : 'catppuccin-mocha';
    this.setTheme(next);
  }

  setTheme(preference: ThemePreference): void {
    this.preference.set(preference);
    if (isPlatformBrowser(this.platformId)) {
      const tid = this.tenantId();
      localStorage.setItem(themeKey(tid), preference);
      // Keep global key in sync for FOUC prevention
      localStorage.setItem('zivue-theme', preference);
      this.applyTheme(this.resolvedTheme());
    }
    this.persistToServer();
  }

  setAccent(accent: AccentColor): void {
    this.accent.set(accent);
    if (isPlatformBrowser(this.platformId)) {
      const tid = this.tenantId();
      localStorage.setItem(accentKey(tid), accent);
      // Keep global key in sync for FOUC prevention
      localStorage.setItem('zivue-accent', accent);
      this.applyAccent(accent);
    }
    this.persistToServer();
  }

  /** Persist current theme/accent to the server if a tenant is active. */
  private persistToServer(): void {
    const tid = this.tenantId();
    if (!tid) return;
    this.tenantApi.updateTenant(tid, {
      theme_preference: this.preference(),
      accent_color: this.accent(),
    }).subscribe({ error: () => { /* silently ignore — localStorage is the fallback */ } });
  }

  private resolve(preference: ThemePreference): string {
    if (preference === 'system') {
      if (isPlatformBrowser(this.platformId)) {
        return window.matchMedia('(prefers-color-scheme: dark)').matches
          ? 'catppuccin-mocha' : 'catppuccin-latte';
      }
      return 'catppuccin-mocha';
    }
    return preference;
  }

  private applyTheme(resolved: string): void {
    const el = this.document.documentElement;
    el.setAttribute('data-theme', resolved);
    CATPPUCCIN_FLAVORS.forEach(f => el.classList.remove(f));
    const flavor = resolved.replace('catppuccin-', '');
    if ((CATPPUCCIN_FLAVORS as readonly string[]).includes(flavor)) {
      el.classList.add(flavor);
    }
    el.style.colorScheme = DARK_THEMES.has(resolved) ? 'dark' : 'light';
  }

  private applyAccent(accent: AccentColor): void {
    this.document.documentElement.setAttribute('data-accent', accent);
  }
}
