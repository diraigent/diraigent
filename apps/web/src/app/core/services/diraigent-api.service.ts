import { Injectable, inject } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable } from 'rxjs';
import { environment } from '../../../environments/environment';

export interface DgPackageInfo {
  id: string;
  slug: string;
  name: string;
}

export interface DgPackage {
  id: string;
  slug: string;
  name: string;
  description: string | null;
  is_builtin: boolean;
  allowed_task_kinds: string[];
  allowed_knowledge_categories: string[];
  allowed_observation_kinds: string[];
  allowed_event_kinds: string[];
  allowed_integration_kinds: string[];
  created_at: string;
  updated_at: string;
}

/** Git topology mode for a project. */
export type DgGitMode = 'monorepo' | 'standalone' | 'none';

export interface DgProject {
  id: string;
  name: string;
  slug: string;
  description: string;
  parent_id: string | null;
  default_playbook_id: string | null;
  package?: DgPackageInfo | null;
  repo_url: string | null;
  /** @deprecated Use project_root instead. Kept for backward compatibility. */
  repo_path: string | null;
  default_branch: string;
  service_name: string | null;
  metadata: Record<string, unknown>;
  created_at: string;
  updated_at: string;
  /** Git topology: 'monorepo' | 'standalone' | 'none' */
  git_mode: DgGitMode;
  /** Path to git repo root (where .git lives), relative to PROJECTS_PATH. Null for git-free projects. */
  git_root: string | null;
  /** Subpath within git_root for the project directory. Only present for monorepo mode. */
  project_root: string | null;
  /** Absolute filesystem path to the project root (server-resolved). */
  resolved_path: string | null;
  /** Absolute filesystem path to the git repository root (server-resolved). */
  git_resolved_path: string | null;
}

export interface DgProjectUpdate {
  name?: string;
  description?: string;
  default_playbook_id?: string | null;
  repo_url?: string | null;
  /** @deprecated Use git_root instead. */
  repo_path?: string | null;
  default_branch?: string;
  service_name?: string | null;
  metadata?: Record<string, unknown>;
  package_slug?: string | null;
  /** Git topology: 'monorepo' | 'standalone' | 'none' */
  git_mode?: DgGitMode;
  /** Path to git repo root (where .git lives), relative to PROJECTS_PATH. */
  git_root?: string | null;
  /** Subpath within git_root for the project directory. Only for monorepo mode. */
  project_root?: string | null;
}

export interface CreateProjectRequest {
  name: string;
  description?: string;
  parent_id?: string;
  repo_url?: string;
  /** @deprecated Use git_root instead. */
  repo_path?: string;
  default_branch?: string;
  service_name?: string;
  package_slug?: string;
  /** Git topology: 'monorepo' | 'standalone' | 'none'. Defaults to 'standalone'. */
  git_mode?: DgGitMode;
  /** Path to git repo root (where .git lives), relative to PROJECTS_PATH. */
  git_root?: string;
  /** Subpath within git_root for the project directory. Only for monorepo mode. */
  project_root?: string;
}

export interface SpHealthResponse {
  status: string;
}

export interface DgSettings {
  projects_path: string | null;
  repo_root: string | null;
}

export interface ClaudeMdResponse {
  content: string;
  exists: boolean;
}

@Injectable({ providedIn: 'root' })
export class DiraigentApiService {
  private http = inject(HttpClient);
  private baseUrl = environment.apiServer;

  getProjects(): Observable<DgProject[]> {
    return this.http.get<DgProject[]>(this.baseUrl);
  }

  getProject(projectId: string): Observable<DgProject> {
    return this.http.get<DgProject>(`${this.baseUrl}/${projectId}`);
  }

  createProject(req: CreateProjectRequest): Observable<DgProject> {
    return this.http.post<DgProject>(this.baseUrl, req);
  }

  updateProject(projectId: string, data: DgProjectUpdate): Observable<DgProject> {
    return this.http.put<DgProject>(`${this.baseUrl}/${projectId}`, data);
  }

  getClaudeMd(projectId: string): Observable<ClaudeMdResponse> {
    return this.http.get<ClaudeMdResponse>(`${this.baseUrl}/${projectId}/claude-md`);
  }

  updateClaudeMd(projectId: string, content: string): Observable<ClaudeMdResponse> {
    return this.http.put<ClaudeMdResponse>(`${this.baseUrl}/${projectId}/claude-md`, { content });
  }

  deleteProject(projectId: string): Observable<void> {
    return this.http.delete<void>(`${this.baseUrl}/${projectId}`);
  }

  getPackages(): Observable<DgPackage[]> {
    return this.http.get<DgPackage[]>(`${this.baseUrl}/packages`);
  }

  getPackage(id: string): Observable<DgPackage> {
    return this.http.get<DgPackage>(`${this.baseUrl}/packages/${id}`);
  }

  getSettings(): Observable<DgSettings> {
    return this.http.get<DgSettings>(`${this.baseUrl}/settings`);
  }

  getHealth(): Observable<SpHealthResponse> {
    const base = new URL(this.baseUrl).origin;
    return this.http.get<SpHealthResponse>(`${base}/health/live`);
  }
}
