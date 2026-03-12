import { Component, Signal, inject, signal } from '@angular/core';
import { Router, RouterLink, RouterLinkActive } from '@angular/router';
import { TranslocoModule } from '@jsverse/transloco';
import { AuthService } from '../../../core/services/auth.service';
import { NavBadgeService } from '../../../core/services/nav-badge.service';
import { ThemeToggleComponent } from '../theme-toggle/theme-toggle';
import { ProjectSwitcherComponent } from '../project-switcher/project-switcher';
import { HealthIndicatorComponent } from '../health-indicator/health-indicator';
import { AgentIndicatorComponent } from '../agent-indicator/agent-indicator';

interface NavItem {
  path: string;
  labelKey: string;
  icon: string;
  /** Optional signal returning the number of pending items to show as a badge */
  badge?: Signal<number>;
}

interface NavGroup {
  labelKey: string;
  icon: string;
  children: NavItem[];
}

type NavEntry = NavItem | NavGroup;

function isNavGroup(entry: NavEntry): entry is NavGroup {
  return 'children' in entry;
}

@Component({
  selector: 'app-sidebar',
  standalone: true,
  imports: [RouterLink, RouterLinkActive, TranslocoModule, ThemeToggleComponent, ProjectSwitcherComponent, HealthIndicatorComponent, AgentIndicatorComponent],
  templateUrl: './sidebar.html',
})
export class SidebarComponent {
  auth = inject(AuthService);
  badges = inject(NavBadgeService);
  private router = inject(Router);
  mobileOpen = signal(false);
  readonly isNavGroup = isNavGroup;

  /** Cast helper for template type safety in @else branch */
  asItem(entry: NavEntry): NavItem {
    return entry as NavItem;
  }

  asGroup(entry: NavEntry): NavGroup {
    return entry as NavGroup;
  }

  /** Track which groups are expanded */
  private expandedGroups = new Set<string>();

  readonly navEntries: NavEntry[] = [
    { path: '/work', labelKey: 'nav.work', icon: 'M13 10V3L4 14h7v7l9-11h-7z' },
    { path: '/review', labelKey: 'nav.review', icon: 'M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z', badge: this.badges.humanReviewCount },
    { path: '/playbooks', labelKey: 'nav.playbooks', icon: 'M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15' },
    { path: '/observations', labelKey: 'nav.observations', icon: 'M15 12a3 3 0 11-6 0 3 3 0 016 0z M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z', badge: this.badges.openObservations },
    {
      labelKey: 'nav.reference',
      icon: 'M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10',
      children: [
        { path: '/decisions', labelKey: 'nav.decisions', icon: 'M3 6l3 1m0 0l-3 9a5.002 5.002 0 006.001 0M6 7l3 9M6 7l6-2m6 2l3-1m-3 1l-3 9a5.002 5.002 0 006.001 0M18 7l3 9m-3-9l-6-2m0-2v2m0 16V5m0 16H9m3 0h3', badge: this.badges.proposedDecisions },
        { path: '/knowledge', labelKey: 'nav.knowledge', icon: 'M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253' },
        { path: '/verifications', labelKey: 'nav.verifications', icon: 'M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z' },
        { path: '/reports', labelKey: 'nav.reports', icon: 'M9 17v-2m3 2v-4m3 4v-6m2 10H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z' },
      ],
    },
    { path: '/integrations', labelKey: 'nav.integrations', icon: 'M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1' },
    { path: '/source', labelKey: 'nav.source', icon: 'M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4' },
    { path: '/audit', labelKey: 'nav.audit', icon: 'M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z' },
    { path: '/settings', labelKey: 'nav.settings', icon: 'M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.573-1.066z M15 12a3 3 0 11-6 0 3 3 0 016 0z' },
  ];

  isGroupExpanded(labelKey: string): boolean {
    return this.expandedGroups.has(labelKey);
  }

  isGroupActive(group: NavGroup): boolean {
    return group.children.some(child => this.router.url.startsWith(child.path));
  }

  toggleGroup(labelKey: string): void {
    if (this.expandedGroups.has(labelKey)) {
      this.expandedGroups.delete(labelKey);
    } else {
      this.expandedGroups.add(labelKey);
    }
  }

  toggleMobile(): void {
    this.mobileOpen.update(v => !v);
  }

  closeMobile(): void {
    this.mobileOpen.set(false);
  }

  login(): void {
    this.auth.login();
  }

  logout(): void {
    this.auth.logout();
  }
}
