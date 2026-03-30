import { Component, inject, AfterViewInit, OnDestroy, signal, effect } from '@angular/core';
import { NavigationEnd, Router, RouterOutlet } from '@angular/router';
import { filter } from 'rxjs/operators';
import { SidebarComponent } from './shared/components/sidebar/sidebar';
import { ChatDrawerComponent } from './features/chat/chat-drawer';
import { AuthService } from './core/services/auth.service';
import { CreateProjectModalComponent } from './shared/components/create-project-modal/create-project-modal';
import { CreateProjectService } from './shared/services/create-project.service';
import { ChatService } from './core/services/chat.service';
import { KeyboardService } from './core/services/keyboard.service';
import { KeyboardHelpComponent } from './shared/components/keyboard-help/keyboard-help';

@Component({
  selector: 'app-root',
  standalone: true,
  imports: [RouterOutlet, SidebarComponent, ChatDrawerComponent, CreateProjectModalComponent, KeyboardHelpComponent],
  template: `
    @if (auth.isLoggedIn()) {
      <app-sidebar #sidebar [class.hidden]="chat.fullscreen()" />
      <div class="h-dvh flex flex-col min-w-0 overflow-x-hidden"
           [class.lg:ml-64]="!chat.fullscreen()">
        <main id="main-content" class="flex-[2] overflow-y-auto overflow-x-hidden min-w-0" tabindex="-1"
              [class.hidden]="chat.fullscreen()">
          <!-- Mobile hamburger bar -->
          <div class="lg:hidden sticky top-0 z-20 flex items-center h-11 px-2 bg-bg-subtle/95 backdrop-blur-sm">
            <button (click)="sidebar.toggleMobile()"
                    class="p-2 rounded-lg text-text-secondary hover:text-text-primary hover:bg-surface-hover transition-colors">
              <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16"/>
              </svg>
            </button>
          </div>
          <router-outlet />
        </main>
        <div id="chat-panel" class="overflow-hidden"
             [class.border-t]="!chat.fullscreen()"
             [class.border-border]="!chat.fullscreen()"
             [class.flex-1]="!chat.collapsed() || chat.fullscreen()"
             [class.min-h-0]="!chat.collapsed() || chat.fullscreen()">
          <app-chat-drawer />
        </div>
      </div>
      <!-- Mobile jump-to-chat FAB: visible only on mobile when chat panel is out of viewport -->
      @if (showChatFab() && !chat.fullscreen()) {
        <button
          (click)="scrollToChat()"
          class="fixed bottom-6 right-6 z-50 lg:hidden w-14 h-14 rounded-full bg-accent text-white shadow-lg
                 flex items-center justify-center transition-all duration-200 hover:opacity-90 active:scale-95"
          aria-label="Jump to chat">
          <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5"
              d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z" />
          </svg>
        </button>
      }
      @if (createProject.isOpen()) {
        <app-create-project-modal
          [parentProjects]="createProject.parentProjects()"
          (created)="createProject.notifyCreated($event)"
          (cancelled)="createProject.close()" />
      }
      @if (keyboard.helpOpen()) {
        <app-keyboard-help />
      }
    } @else if (!auth.isAuthInitialized()) {
      <div class="min-h-screen flex items-center justify-center bg-bg-subtle">
        <div class="text-center space-y-6">
          <h1 class="text-3xl font-semibold text-accent">Diraigent</h1>
          <p class="text-text-secondary">Loading...</p>
        </div>
      </div>
    } @else if (isLandingRoute()) {
      <router-outlet />
    } @else {
      <div class="min-h-screen flex items-center justify-center bg-bg-subtle">
        <div class="text-center space-y-6">
          <h1 class="text-3xl font-semibold text-accent">Diraigent</h1>
          <p class="text-text-secondary">Sign in to access the dashboard</p>
          <div class="flex gap-3 justify-center">
            <button (click)="auth.login()"
                    class="px-6 py-2 rounded-lg bg-accent text-white hover:opacity-90 transition-opacity">
              Login
            </button>
            <a [href]="auth.registrationUrl"
               class="px-6 py-2 rounded-lg border border-border text-text-secondary hover:bg-bg-muted transition-colors">
              Register
            </a>
          </div>
        </div>
      </div>
    }
  `,
})
export class App implements AfterViewInit, OnDestroy {
  private router = inject(Router);
  auth = inject(AuthService);
  createProject = inject(CreateProjectService);
  chat = inject(ChatService);
  keyboard = inject(KeyboardService);

  /** True when the current URL is the landing page (root path). */
  isLandingRoute = signal(false);

  /** Whether the floating chat button should be visible (mobile only, chat out of viewport). */
  showChatFab = signal(false);

  private observer: IntersectionObserver | null = null;
  private desktopQuery = window.matchMedia('(min-width: 1024px)');
  private onDesktopChange = (e: MediaQueryListEvent) => {
    if (e.matches) this.showChatFab.set(false);
  };
  private detachKeyboard: (() => void) | null = null;

  constructor() {
    // Track whether we're on the landing page
    this.isLandingRoute.set(this.router.url === '/');
    this.router.events
      .pipe(filter((e): e is NavigationEnd => e instanceof NavigationEnd))
      .subscribe(e => this.isLandingRoute.set(e.urlAfterRedirects === '/'));

    // Attach global keyboard shortcuts
    this.detachKeyboard = this.keyboard.attach();

    // React to scrollToChat signal from ChatService (e.g. openWithMessage from goals/tasks)
    effect(() => {
      if (this.chat.scrollToChat()) {
        this.scrollToChat();
        this.chat.scrollToChat.set(false);
      }
    });
  }

  ngAfterViewInit(): void {
    this.setupChatObserver();
  }

  ngOnDestroy(): void {
    this.observer?.disconnect();
    this.observer = null;
    this.desktopQuery.removeEventListener('change', this.onDesktopChange);
    this.detachKeyboard?.();
  }

  /** Smooth-scroll the chat panel into view. */
  scrollToChat(): void {
    const chatPanel = document.getElementById('chat-panel');
    chatPanel?.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }

  private setupChatObserver(): void {
    const chatPanel = document.getElementById('chat-panel');
    if (!chatPanel) return;

    this.observer = new IntersectionObserver(
      ([entry]) => {
        // Show FAB only when chat panel is NOT intersecting AND we're on a mobile viewport
        this.showChatFab.set(!entry.isIntersecting && !this.desktopQuery.matches);
      },
      { root: null, threshold: 0 },
    );

    this.observer.observe(chatPanel);

    // Hide FAB immediately when viewport grows past the lg breakpoint
    this.desktopQuery.addEventListener('change', this.onDesktopChange);
  }
}
