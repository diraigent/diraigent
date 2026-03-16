import { Injectable, inject } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable } from 'rxjs';
import { environment } from '../../../environments/environment';

// ── Models ──

export interface Tenant {
  id: string;
  name: string;
  slug: string;
  encryption_mode: 'none' | 'login_derived' | 'passphrase';
  key_salt: string | null;
  theme_preference: string;
  accent_color: string;
  created_at: string;
  updated_at: string;
}

export interface TenantMember {
  id: string;
  tenant_id: string;
  user_id: string;
  role: 'owner' | 'admin' | 'member';
  created_at: string;
  updated_at: string;
}

export interface WrappedKey {
  id: string;
  tenant_id: string;
  user_id: string;
  key_type: 'login_derived' | 'passphrase';
  wrapped_dek: string;
  kdf_salt: string;
  kdf_params: Record<string, unknown> | null;
  key_version: number;
  created_at: string;
}

export interface CreateTenantRequest {
  name: string;
  slug: string;
}

export interface UpdateTenantRequest {
  name?: string;
  encryption_mode?: string;
  key_salt?: string;
  theme_preference?: string;
  accent_color?: string;
}

export interface InitEncryptionResponse {
  encryption_mode: string;
  salt: string;
  wrapped_dek: string;
  kdf_salt: string;
}

export interface EncryptionSaltResponse {
  encryption_mode: string;
  salt: string | null;
}

// ── Service ──

@Injectable({ providedIn: 'root' })
export class TenantApiService {
  private http = inject(HttpClient);
  private baseUrl = environment.apiServer;

  // ── Tenant CRUD ──

  createTenant(req: CreateTenantRequest): Observable<Tenant> {
    return this.http.post<Tenant>(`${this.baseUrl}/tenants`, req);
  }

  listTenants(): Observable<Tenant[]> {
    return this.http.get<Tenant[]>(`${this.baseUrl}/tenants`);
  }

  getTenant(tenantId: string): Observable<Tenant> {
    return this.http.get<Tenant>(`${this.baseUrl}/tenants/${tenantId}`);
  }

  getTenantBySlug(slug: string): Observable<Tenant> {
    return this.http.get<Tenant>(`${this.baseUrl}/tenants/by-slug/${slug}`);
  }

  getMyTenant(): Observable<Tenant | null> {
    return this.http.get<Tenant | null>(`${this.baseUrl}/tenants/me`);
  }

  updateTenant(tenantId: string, data: UpdateTenantRequest): Observable<Tenant> {
    return this.http.put<Tenant>(`${this.baseUrl}/tenants/${tenantId}`, data);
  }

  deleteTenant(tenantId: string): Observable<void> {
    return this.http.delete<void>(`${this.baseUrl}/tenants/${tenantId}`);
  }

  // ── Members ──

  listMembers(tenantId: string): Observable<TenantMember[]> {
    return this.http.get<TenantMember[]>(`${this.baseUrl}/tenants/${tenantId}/members`);
  }

  addMember(tenantId: string, userId: string, role?: string): Observable<TenantMember> {
    return this.http.post<TenantMember>(`${this.baseUrl}/tenants/${tenantId}/members`, {
      user_id: userId,
      role,
    });
  }

  removeMember(tenantId: string, memberId: string): Observable<void> {
    return this.http.delete<void>(`${this.baseUrl}/tenants/${tenantId}/members/${memberId}`);
  }

  // ── Encryption ──

  /** Initialize login-derived encryption for a tenant. */
  initEncryption(tenantId: string, accessToken: string): Observable<InitEncryptionResponse> {
    return this.http.post<InitEncryptionResponse>(
      `${this.baseUrl}/tenants/${tenantId}/encryption/init`,
      { access_token: accessToken },
    );
  }

  /** Get the encryption salt and mode for a tenant. */
  getEncryptionSalt(tenantId: string): Observable<EncryptionSaltResponse> {
    return this.http.get<EncryptionSaltResponse>(
      `${this.baseUrl}/tenants/${tenantId}/encryption/salt`,
    );
  }

  /** Get the raw DEK (base64) for orchestra configuration. Owner-only. */
  getDekForOrchestra(tenantId: string): Observable<{ dek: string }> {
    return this.http.get<{ dek: string }>(
      `${this.baseUrl}/tenants/${tenantId}/encryption/dek`,
    );
  }

  /** Rotate the tenant's encryption key. Requires encryption to be unlocked. */
  rotateKeys(tenantId: string, accessToken: string): Observable<{ new_key_version: number; fields_rotated: number }> {
    return this.http.post<{ new_key_version: number; fields_rotated: number }>(
      `${this.baseUrl}/tenants/${tenantId}/encryption/rotate`,
      { access_token: accessToken },
    );
  }

  /** Unlock encryption by providing the access token (login-derived mode). */
  unlockEncryption(tenantId: string, accessToken: string): Observable<{ status: string }> {
    return this.http.post<{ status: string }>(
      `${this.baseUrl}/tenants/${tenantId}/encryption/unlock`,
      { access_token: accessToken },
    );
  }

  // ── Wrapped Keys ──

  listKeys(tenantId: string, userId: string): Observable<WrappedKey[]> {
    return this.http.get<WrappedKey[]>(
      `${this.baseUrl}/tenants/${tenantId}/members/${userId}/keys`,
    );
  }

  createKey(
    tenantId: string,
    userId: string,
    data: { key_type: string; wrapped_dek: string; kdf_salt: string; key_version?: number },
  ): Observable<WrappedKey> {
    return this.http.post<WrappedKey>(
      `${this.baseUrl}/tenants/${tenantId}/members/${userId}/keys`,
      data,
    );
  }

  deleteKey(tenantId: string, keyId: string): Observable<void> {
    return this.http.delete<void>(`${this.baseUrl}/tenants/${tenantId}/keys/${keyId}`);
  }
}
