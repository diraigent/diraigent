import { Component, inject, viewChild, ElementRef, effect, untracked } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ChatService } from '../../core/services/chat.service';

@Component({
  selector: 'app-chat-drawer',
  standalone: true,
  imports: [FormsModule],
  styles: [`:host { display: contents; }`],
  template: `
    <!-- Floating action button -->
    <button
      (click)="toggle()"
      class="fixed bottom-11 right-6 z-[60] w-14 h-14 rounded-full bg-accent text-white shadow-lg
             hover:opacity-90 transition-all flex items-center justify-center"
      [class.hidden]="chat.isOpen()">
      <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
        <path stroke-linecap="round" stroke-linejoin="round"
              d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z" />
      </svg>
    </button>

    <!-- Drawer -->
    @if (chat.isOpen()) {
      <div class="fixed bottom-11 right-6 z-[60] flex flex-col
                  w-[840px] max-w-[calc(100vw-2rem)] h-[min(80vh,800px)]
                  rounded-2xl shadow-2xl border border-border bg-surface overflow-hidden">

        <!-- Header -->
        <div class="flex items-center justify-between px-4 py-3 border-b border-border bg-bg-subtle">
          <span class="font-semibold text-text-primary">AI Assistant</span>
          <div class="flex gap-2">
            <button (click)="chat.clear()" class="text-xs text-text-secondary hover:text-text-primary transition-colors">
              Clear
            </button>
            <button (click)="toggle()" class="text-text-secondary hover:text-text-primary transition-colors">
              <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>
        </div>

        <!-- Messages -->
        <div #messageList class="flex-1 overflow-y-auto p-4 space-y-3" (scroll)="onScroll()">
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

          <!-- Thinking indicator -->
          @if (chat.streaming() && !chat.streamingText() && chat.activeTools().length === 0) {
            <div class="flex justify-start">
              <div class="rounded-2xl rounded-bl-md px-4 py-2 bg-bg-subtle text-text-secondary text-sm animate-pulse">
                Thinking...
              </div>
            </div>
          }

          <!-- Tool status -->
          @for (tool of chat.activeTools(); track tool.toolId) {
            <div class="flex items-center gap-2 text-xs text-text-secondary px-1">
              <span [class]="{
                'w-2 h-2 rounded-full': true,
                'bg-ctp-yellow animate-pulse': tool.status === 'running',
                'bg-ctp-green': tool.status === 'success',
                'bg-ctp-red': tool.status === 'error'
              }"></span>
              {{ tool.toolName }}
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
    }
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
      const isOpen = this.chat.isOpen();

      untracked(() => {
        // Whenever the drawer opens, reset scroll tracking and jump to bottom.
        if (isOpen) {
          this.userScrolledUp = false;
        }
        if (!this.userScrolledUp) {
          // Defer one microtask so Angular has flushed DOM changes first.
          setTimeout(() => this.scrollToBottom(), 0);
        }
      });
    });
  }

  toggle(): void {
    this.chat.isOpen.update(v => !v);
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
