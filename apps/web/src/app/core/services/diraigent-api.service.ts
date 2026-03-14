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

export interface TokenDayCount {
  day: string;
  input_tokens: number;
  output_tokens: number;
  cost_usd: number;
}

export interface TaskCostRow {
  task_id: string;
  task_number: number;
  title: string;
  state: string;
  input_tokens: number;
  output_tokens: number;
  cost_usd: number;
}

export interface CostSummary {
  total_input_tokens: number;
  total_output_tokens: number;
  total_cost_usd: number;
}

export interface TaskSummary {
  total: number;
  done: number;
  cancelled: number;
  in_progress: number;
  ready: number;
  backlog: number;
  human_review: number;
}

export interface ProjectMetrics {
  project_id: string;
  range_days: number;
  task_summary: TaskSummary;
  tasks_per_day: { day: string; count: number }[];
  avg_time_in_state_hours: { state: string; avg_hours: number | null }[];
  agent_breakdown: { agent_id: string; agent_name: string; tasks_completed: number; tasks_in_progress: number; avg_completion_hours: number | null }[];
  playbook_completion: { playbook_id: string; playbook_title: string; total_tasks: number; completed_tasks: number; completion_rate: number }[];
  cost_summary: CostSummary;
  task_costs: TaskCostRow[];
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

  getProjectMetrics(projectId: string, days?: number): Observable<ProjectMetrics> {
    const params: Record<string, string> = {};
    if (days != null) {
      params['days'] = days.toString();
    }
    return this.http.get<ProjectMetrics>(`${this.baseUrl}/${projectId}/metrics`, { params });
  }
}
