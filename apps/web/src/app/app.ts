import { Component, inject, AfterViewInit, OnDestroy, signal, effect } from '@angular/core';
import { RouterOutlet } from '@angular/router';
import { SidebarComponent } from './shared/components/sidebar/sidebar';
import { ChatDrawerComponent } from './features/chat/chat-drawer';
import { AuthService } from './core/services/auth.service';
import { CreateProjectModalComponent } from './shared/components/create-project-modal/create-project-modal';
import { CreateProjectService } from './shared/services/create-project.service';
import { ChatService } from './core/services/chat.service';

@Component({
  selector: 'app-root',
  standalone: true,
  imports: [RouterOutlet, SidebarComponent, ChatDrawerComponent, CreateProjectModalComponent],
  template: `
    @if (auth.isLoggedIn()) {
      <app-sidebar />
      <div class="lg:ml-64 h-screen flex flex-col">
        <main id="main-content" class="flex-[2] overflow-y-auto pt-14 lg:pt-0" tabindex="-1">
          <router-outlet />
        </main>
        <div id="chat-panel" class="border-t border-border overflow-hidden"
             [class.flex-1]="!chat.collapsed()"
             [class.min-h-0]="!chat.collapsed()">
          <app-chat-drawer />
        </div>
      </div>
      <!-- Mobile jump-to-chat FAB: visible only on mobile when chat panel is out of viewport -->
      @if (showChatFab()) {
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
    } @else if (!auth.isAuthInitialized()) {
      <div class="min-h-screen flex items-center justify-center bg-bg-subtle">
        <div class="text-center space-y-6">
          <h1 class="text-3xl font-semibold text-accent">Diraigent</h1>
          <p class="text-text-secondary">Loading...</p>
        </div>
      </div>
    } @else {
      <div class="min-h-screen flex items-center justify-center bg-bg-subtle">
        <div class="text-center space-y-6">
          <h1 class="text-3xl font-semibold text-accent">Diraigent</h1>
          <p class="text-text-secondary">Sign in to access the dashboard</p>
          <button (click)="auth.login()"
                  class="px-6 py-2 rounded-lg bg-accent text-white hover:opacity-90 transition-opacity">
            Login
          </button>
        </div>
      </div>
      <router-outlet />
    }
  `,
})
export class App implements AfterViewInit, OnDestroy {
  auth = inject(AuthService);
  createProject = inject(CreateProjectService);
  chat = inject(ChatService);

  /** Whether the floating chat button should be visible (mobile only, chat out of viewport). */
  showChatFab = signal(false);

  private observer: IntersectionObserver | null = null;
  private desktopQuery = window.matchMedia('(min-width: 1024px)');
  private onDesktopChange = (e: MediaQueryListEvent) => {
    if (e.matches) this.showChatFab.set(false);
  };

  constructor() {
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
