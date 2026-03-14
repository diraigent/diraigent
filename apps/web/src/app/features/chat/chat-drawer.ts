import { Component, inject, viewChild, ElementRef, effect, untracked } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ChatService } from '../../core/services/chat.service';

@Component({
  selector: 'app-chat-drawer',
  standalone: true,
  imports: [FormsModule],
  styles: [`:host { display: block; height: 100%; }`],
  template: `
      <div class="flex flex-col bg-surface overflow-hidden"
           [class.h-full]="!chat.collapsed()">

        <!-- Header -->
        <div class="flex items-center justify-between px-4 py-3 border-b border-border bg-bg-subtle">
          <span class="font-semibold text-text-primary">AI Assistant</span>
          <div class="flex items-center gap-2">
            @if (!chat.collapsed()) {
              <button (click)="chat.clear()"
                      class="text-xs text-text-secondary hover:text-text-primary transition-colors">
                Clear
              </button>
            }
            <button (click)="chat.toggleCollapsed()"
                    class="p-1 text-text-secondary hover:text-text-primary transition-colors"
                    [attr.aria-label]="chat.collapsed() ? 'Expand chat' : 'Collapse chat'">
              <svg class="w-4 h-4 transition-transform duration-300"
                   [class.rotate-180]="chat.collapsed()"
                   fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
              </svg>
            </button>
          </div>
        </div>

        <!-- Collapsible content -->
        <div class="grid min-h-0 transition-[grid-template-rows] duration-300 ease-in-out"
             [style.gridTemplateRows]="chat.collapsed() ? '0fr' : '1fr'">
          <div class="overflow-hidden flex flex-col min-h-0">

            <!-- Messages -->
            <div #messageList class="flex-1 overflow-y-auto min-h-0 p-4 space-y-3" (scroll)="onScroll()">
              @if (!chat.canSend()) {
                <div class="flex flex-col items-center justify-center h-full text-center px-6">
                  <div class="text-text-muted text-sm">
                    <p class="font-medium text-text-secondary mb-1">No project selected</p>
                    <p>Create or select a project to start chatting with the AI assistant.</p>
                  </div>
                </div>
              }
              @for (msg of chat.messages(); track $index) {
                <div [class]="msg.role === 'user' ? 'flex justify-end' : 'flex justify-start'">
                  <div [class]="msg.role === 'user'
                    ? 'max-w-[85%] rounded-2xl rounded-br-md px-4 py-2 bg-accent text-bg text-sm'
                    : 'max-w-[85%] rounded-2xl rounded-bl-md px-4 py-2 bg-bg-subtle text-text-primary text-sm whitespace-pre-wrap'">
                    {{ msg.content }}
                  </div>
                </div>
              }

              <!-- Streaming text -->
              @if (chat.streaming() && chat.streamingText()) {
                <div class="flex justify-start">
                  <div class="max-w-[85%] rounded-2xl rounded-bl-md px-4 py-2 bg-bg-subtle text-text-primary text-sm whitespace-pre-wrap">
                    {{ chat.streamingText() }}
                    <span class="inline-block w-1.5 h-4 bg-accent animate-pulse ml-0.5 align-text-bottom"></span>
                  </div>
                </div>
              }

              <!-- Thinking indicator (only before any tools have run) -->
              @if (chat.streaming() && !chat.streamingText() && chat.activeTools().length === 0 && chat.toolsCompleted() === 0) {
                <div class="flex justify-start">
                  <div class="rounded-2xl rounded-bl-md px-4 py-2 bg-bg-subtle text-text-secondary text-sm animate-pulse">
                    Thinking...
                  </div>
                </div>
              }

              <!-- Tool activity indicator (collapsed spinner) -->
              @if (chat.streaming() && (chat.activeTools().length > 0 || chat.toolsCompleted() > 0)) {
                <div class="flex items-center gap-2 text-xs text-text-secondary px-1 py-1">
                  @if (chat.activeTools().length > 0) {
                    <svg class="w-3.5 h-3.5 animate-spin text-accent" viewBox="0 0 24 24" fill="none">
                      <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                      <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                    </svg>
                    <span>{{ chat.activeTools()[chat.activeTools().length - 1].toolName }}</span>
                  }
                  @if (chat.toolsCompleted() > 0) {
                    <span class="text-text-muted">({{ chat.toolsCompleted() }} tool calls)</span>
                  }
                </div>
              }

              <!-- Error -->
              @if (chat.error()) {
                <div class="text-sm text-ctp-red px-1">{{ chat.error() }}</div>
              }
            </div>

            <!-- Input -->
            <div class="border-t border-border p-3">
              <div class="flex gap-2">
                <textarea
                  #inputEl
                  [(ngModel)]="inputText"
                  (keydown.enter)="onEnter($event)"
                  [placeholder]="chat.canSend() ? 'Ask about your project...' : 'Select a project first'"
                  [disabled]="!chat.canSend()"
                  rows="1"
                  class="flex-1 resize-none rounded-xl border border-border bg-bg-subtle px-3 py-2
                         text-sm text-text-primary placeholder:text-text-secondary
                         focus:outline-none focus:ring-1 focus:ring-accent"></textarea>
                @if (chat.streaming()) {
                  <button (click)="chat.cancel()"
                          class="self-end px-3 py-2 rounded-xl bg-ctp-red/10 text-ctp-red text-sm hover:bg-ctp-red/20 transition-colors">
                    Stop
                  </button>
                }
                <button (click)="sendMessage()"
                        [disabled]="!inputText.trim() || !chat.canSend()"
                        class="self-end px-3 py-2 rounded-xl bg-accent text-bg text-sm hover:opacity-90 transition-opacity
                               disabled:opacity-40 disabled:cursor-not-allowed">
                  Send
                </button>
              </div>
            </div>

          </div>
        </div>
      </div>
  `,
})
export class ChatDrawerComponent {
  chat = inject(ChatService);
  inputText = '';

  private messageList = viewChild<ElementRef<HTMLDivElement>>('messageList');
  /** Set to true when the user manually scrolls up to read history. */
  private userScrolledUp = false;

  constructor() {
    // Auto-scroll whenever messages or streaming content change,
    // unless the user has deliberately scrolled up to read history.
    effect(() => {
      this.chat.messages();
      this.chat.streamingText();

      untracked(() => {
        if (!this.userScrolledUp) {
          // Defer one microtask so Angular has flushed DOM changes first.
          setTimeout(() => this.scrollToBottom(), 0);
        }
      });
    });
  }

  /**
   * Track manual scrolling so we don't hijack the user while they read history.
   * Resets automatically when they send a new message.
   */
  onScroll(): void {
    const el = this.messageList()?.nativeElement;
    if (!el) return;
    // Consider "at the bottom" if within 80px of the scroll end.
    const distanceFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
    this.userScrolledUp = distanceFromBottom > 80;
  }

  onEnter(event: Event): void {
    const ke = event as KeyboardEvent;
    if (ke.shiftKey) return; // allow newlines
    ke.preventDefault();
    this.sendMessage();
  }

  async sendMessage(): Promise<void> {
    const text = this.inputText.trim();
    if (!text) return;
    this.chat.cancel();
    this.inputText = '';
    // Sending a new message always resets scroll tracking.
    this.userScrolledUp = false;
    await this.chat.send(text);
  }

  private scrollToBottom(): void {
    const el = this.messageList()?.nativeElement;
    if (el) el.scrollTop = el.scrollHeight;
  }
}
