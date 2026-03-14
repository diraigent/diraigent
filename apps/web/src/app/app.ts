import { Component, inject } from '@angular/core';
import { RouterOutlet } from '@angular/router';
import { SidebarComponent } from './shared/components/sidebar/sidebar';
import { ChatDrawerComponent } from './features/chat/chat-drawer';
import { AuthService } from './core/services/auth.service';
import { CreateProjectModalComponent } from './shared/components/create-project-modal/create-project-modal';
import { CreateProjectService } from './shared/services/create-project.service';

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
        <div id="chat-panel" class="flex-1 min-h-0 border-t border-border">
          <app-chat-drawer />
        </div>
      </div>
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
export class App {
  auth = inject(AuthService);
  createProject = inject(CreateProjectService);
}
