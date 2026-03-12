import { Injectable, signal } from '@angular/core';
import { Subject } from 'rxjs';
import { DgProject } from '../../core/services/diraigent-api.service';

@Injectable({ providedIn: 'root' })
export class CreateProjectService {
  /** Whether the create-project modal is open */
  isOpen = signal(false);

  /** Parent projects list for the parent-project dropdown */
  parentProjects = signal<DgProject[]>([]);

  /** Emits the newly created project so callers can react */
  readonly projectCreated$ = new Subject<DgProject>();

  open(parentProjects: DgProject[] = []): void {
    this.parentProjects.set(parentProjects);
    this.isOpen.set(true);
  }

  close(): void {
    this.isOpen.set(false);
  }

  notifyCreated(project: DgProject): void {
    this.projectCreated$.next(project);
    this.close();
  }
}
