import {
  AuditLogItem,
  AuditLogListResponse,
  AuditLogQuery,
  CreatePlanRequest,
  CreateTenantRequest,
  CsrfResponse,
  HardeningApplyRequest,
  HardeningApplyResponse,
  HardeningStatus,
  HealthResponse,
  JobItem,
  JobListResponse,
  JobProgressEvent,
  LoginResponse,
  Plan,
  PlanListResponse,
  PlanResponse,
  SystemInfoResponse,
  TenantDetailResponse,
  TenantListItem,
  TenantListResponse,
  TenantResourceUsage,
  TwoFactorEnableResponse,
  TwoFactorSetupResponse,
  TwoFactorStatusResponse,
  UpdatePlanRequest,
  User,
  Site,
  SiteDomain,
  CreateSiteRequest,
  AddDomainRequest,
  UpdateDomainRequest,
  SiteMessageResponse,
  SslStatus,
  IssueSslRequest,
  CustomCertRequest,
  SslActionResponse,
  SiteLogsResponse,
  PhpRuntimeListApiResponse,
  PhpExtensionActionApiResponse,
  PhpExtensionListApiResponse,
  SitePhpExtensionsResponse,
  SwitchPhpVersionResponse,
  SitePhpSettingsResponse,
  UpdateSitePhpSettingsInput,
  SitePhpStatusResponse,
  SitePhpLogsResponse,
} from '../types/api';
import type { FsListResponse, TrashListResponse } from '../types/files';

const API_BASE = '';
let cachedCsrfToken: string | null = null;

export function setCsrfToken(token: string | null) {
  cachedCsrfToken = token;
}

export function getCsrfToken(): string | null {
  return cachedCsrfToken;
}

async function request<T>(
  url: string,
  options: RequestInit = {}
): Promise<T> {
  const headers = new Headers(options.headers || {});
  
  if (!headers.has('Content-Type') && options.body && typeof options.body === 'string') {
    headers.set('Content-Type', 'application/json');
  }

  // Attach CSRF token on mutating methods if available
  const method = (options.method || 'GET').toUpperCase();
  if (['POST', 'PUT', 'DELETE', 'PATCH'].includes(method) && cachedCsrfToken) {
    headers.set('X-CSRF-Token', cachedCsrfToken);
  }

  const res = await fetch(`${API_BASE}${url}`, {
    ...options,
    headers,
    credentials: 'include',
  });

  if (!res.ok) {
    let errorMessage = `Request failed: ${res.statusText}`;
    try {
      const errJson = await res.json();
      if (errJson && errJson.message) {
        errorMessage = errJson.message;
      }
    } catch {
      try {
        const text = await res.text();
        if (text) errorMessage = text;
      } catch {
        // use default message
      }
    }
    const err = new Error(errorMessage);
    (err as unknown as { status: number }).status = res.status;
    throw err;
  }

  // Handle empty bodies (e.g. 204 No Content)
  if (res.status === 204) {
    return {} as T;
  }

  return res.json();
}

// System & Health
export async function fetchHealth(): Promise<HealthResponse> {
  return request<HealthResponse>('/healthz');
}

export async function fetchSystemInfo(): Promise<SystemInfoResponse> {
  return request<SystemInfoResponse>('/api/v1/system/info');
}

// Authentication & Session
export async function fetchCsrfToken(): Promise<string> {
  const res = await request<CsrfResponse>('/api/v1/auth/csrf');
  setCsrfToken(res.csrf_token);
  return res.csrf_token;
}

export async function fetchMe(): Promise<User> {
  const res = await request<LoginResponse | User>('/api/v1/auth/me');
  if ('user' in res && res.user) {
    return res.user;
  }
  return res as User;
}

export async function login(
  username: string,
  password: string,
  twoFactorCode?: string
): Promise<LoginResponse> {
  const body: Record<string, string> = { username, password };
  if (twoFactorCode) {
    body.two_factor_code = twoFactorCode;
  }

  const res = await request<LoginResponse>('/api/v1/auth/login', {
    method: 'POST',
    body: JSON.stringify(body),
  });

  if (res.csrf_token) {
    setCsrfToken(res.csrf_token);
  }

  return res;
}

export async function logout(): Promise<{ message: string }> {
  const res = await request<{ message: string }>('/api/v1/auth/logout', {
    method: 'POST',
  });
  setCsrfToken(null);
  return res;
}

export async function changePassword(
  oldPassword: string,
  newPassword: string
): Promise<{ message: string; csrf_token: string }> {
  const res = await request<{ message: string; csrf_token: string }>(
    '/api/v1/auth/password',
    {
      method: 'POST',
      body: JSON.stringify({
        old_password: oldPassword,
        new_password: newPassword,
      }),
    }
  );
  if (res.csrf_token) {
    setCsrfToken(res.csrf_token);
  }
  return res;
}

// Two-Factor Authentication (2FA)
export async function setup2FA(): Promise<TwoFactorSetupResponse> {
  return request<TwoFactorSetupResponse>('/api/v1/auth/2fa/setup', {
    method: 'POST',
  });
}

export async function enable2FA(code: string): Promise<TwoFactorEnableResponse> {
  return request<TwoFactorEnableResponse>('/api/v1/auth/2fa/enable', {
    method: 'POST',
    body: JSON.stringify({ code }),
  });
}

export async function disable2FA(code: string): Promise<{ message: string }> {
  return request<{ message: string }>('/api/v1/auth/2fa/disable', {
    method: 'POST',
    body: JSON.stringify({ code }),
  });
}

export async function fetch2FAStatus(): Promise<TwoFactorStatusResponse> {
  return request<TwoFactorStatusResponse>('/api/v1/auth/2fa/status');
}

// Audit Logs
export async function fetchAuditLogs(
  query: AuditLogQuery = {}
): Promise<AuditLogListResponse> {
  const params = new URLSearchParams();
  if (query.page) params.set('page', query.page.toString());
  if (query.per_page) params.set('per_page', query.per_page.toString());
  if (query.action) params.set('action', query.action);
  if (query.actor) params.set('actor', query.actor);
  if (query.target) params.set('target', query.target);
  if (query.status) params.set('status', query.status);

  const qs = params.toString() ? `?${params.toString()}` : '';
  return request<AuditLogListResponse>(`/api/v1/audit/logs${qs}`);
}

export async function fetchAuditLogDetail(id: number): Promise<AuditLogItem> {
  return request<AuditLogItem>(`/api/v1/audit/logs/${id}`);
}

// Background Jobs
export async function fetchJobs(
  query: { status?: string; page?: number; per_page?: number } = {}
): Promise<JobListResponse> {
  const params = new URLSearchParams();
  if (query.page) params.set('page', query.page.toString());
  if (query.per_page) params.set('per_page', query.per_page.toString());
  if (query.status) params.set('status', query.status);

  const qs = params.toString() ? `?${params.toString()}` : '';
  return request<JobListResponse>(`/api/v1/jobs${qs}`);
}

export async function fetchJobDetail(id: string): Promise<JobItem> {
  return request<JobItem>(`/api/v1/jobs/${id}`);
}

export async function cancelJob(id: string): Promise<JobItem> {
  return request<JobItem>(`/api/v1/jobs/${id}/cancel`, {
    method: 'POST',
  });
}

export function subscribeJobEvents(
  jobId: string,
  onEvent: (event: JobProgressEvent) => void,
  onError?: (err: Event) => void
): () => void {
  const eventSource = new EventSource(`${API_BASE}/api/v1/jobs/${jobId}/events`, {
    withCredentials: true,
  });

  eventSource.addEventListener('progress', (e: MessageEvent) => {
    try {
      const data: JobProgressEvent = JSON.parse(e.data);
      onEvent(data);
      if (['succeeded', 'failed', 'cancelled'].includes(data.status)) {
        eventSource.close();
      }
    } catch {
      // parse error
    }
  });

  if (onError) {
    eventSource.onerror = (err) => {
      onError(err);
    };
  }

  return () => {
    eventSource.close();
  };
}

// Plans API
export async function fetchPlans(): Promise<Plan[]> {
  const res = await request<PlanListResponse>('/api/v1/plans');
  return res.plans;
}

export async function fetchPlan(id: number): Promise<Plan> {
  const res = await request<PlanResponse>(`/api/v1/plans/${id}`);
  return res.plan;
}

export async function createPlan(data: CreatePlanRequest): Promise<Plan> {
  const res = await request<PlanResponse>('/api/v1/plans', {
    method: 'POST',
    body: JSON.stringify(data),
  });
  return res.plan;
}

export async function updatePlan(id: number, data: UpdatePlanRequest): Promise<Plan> {
  const res = await request<PlanResponse>(`/api/v1/plans/${id}`, {
    method: 'PUT',
    body: JSON.stringify(data),
  });
  return res.plan;
}

export async function deletePlan(id: number): Promise<{ success: boolean; message: string }> {
  return request<{ success: boolean; message: string }>(`/api/v1/plans/${id}`, {
    method: 'DELETE',
  });
}

// Tenants API
export async function fetchTenants(query?: { search?: string; status?: string }): Promise<TenantListItem[]> {
  const params = new URLSearchParams();
  if (query?.search) params.append('search', query.search);
  if (query?.status) params.append('status', query.status);
  const q = params.toString() ? `?${params.toString()}` : '';
  const res = await request<TenantListResponse>(`/api/v1/tenants${q}`);
  return res.tenants;
}

export async function fetchTenantDetail(id: number): Promise<TenantDetailResponse> {
  return request<TenantDetailResponse>(`/api/v1/tenants/${id}`);
}

export async function createTenant(data: CreateTenantRequest): Promise<TenantDetailResponse> {
  return request<TenantDetailResponse>('/api/v1/tenants', {
    method: 'POST',
    body: JSON.stringify(data),
  });
}

export async function fetchTenantResources(id: number): Promise<TenantResourceUsage> {
  return request<TenantResourceUsage>(`/api/v1/tenants/${id}/resources`);
}

export async function suspendTenant(id: number): Promise<{ success: boolean; message: string }> {
  return request<{ success: boolean; message: string }>(`/api/v1/tenants/${id}/suspend`, {
    method: 'POST',
  });
}

export async function unsuspendTenant(id: number): Promise<{ success: boolean; message: string }> {
  return request<{ success: boolean; message: string }>(`/api/v1/tenants/${id}/unsuspend`, {
    method: 'POST',
  });
}

export async function deleteTenant(id: number, purge?: boolean): Promise<{ success: boolean; message: string }> {
  const q = purge ? '?purge=true' : '';
  return request<{ success: boolean; message: string }>(`/api/v1/tenants/${id}${q}`, {
    method: 'DELETE',
  });
}

export async function impersonateTenant(id: number): Promise<User> {
  const res = await request<{ user: User; csrf_token: string; impersonated_by: string }>(`/api/v1/tenants/${id}/impersonate`, {
    method: 'POST',
  });
  if (res.csrf_token) {
    setCsrfToken(res.csrf_token);
  }
  return res.user;
}

export async function stopImpersonating(): Promise<void> {
  await request<{ success: boolean; message: string }>('/api/v1/auth/stop-impersonating', {
    method: 'POST',
  });
}

// System Hardening API
export async function fetchHardeningStatus(): Promise<HardeningStatus> {
  return request<HardeningStatus>('/api/v1/system/hardening');
}

export async function applyHardening(data: HardeningApplyRequest): Promise<HardeningApplyResponse> {
  return request<HardeningApplyResponse>('/api/v1/system/hardening/apply', {
    method: 'POST',
    body: JSON.stringify(data),
  });
}

// ==========================================
// Sites API
// ==========================================

export async function fetchSites(query?: { user_id?: number }): Promise<Site[]> {
  const params = new URLSearchParams();
  if (query?.user_id) {
    params.set('user_id', query.user_id.toString());
  }
  const queryString = params.toString() ? `?${params.toString()}` : '';
  return request<Site[]>(`/api/v1/sites${queryString}`);
}

export async function fetchSite(id: number): Promise<Site> {
  return request<Site>(`/api/v1/sites/${id}`);
}

export async function createSite(data: CreateSiteRequest): Promise<Site> {
  return request<Site>('/api/v1/sites', {
    method: 'POST',
    body: JSON.stringify(data),
  });
}

export async function deleteSite(id: number): Promise<SiteMessageResponse> {
  return request<SiteMessageResponse>(`/api/v1/sites/${id}`, {
    method: 'DELETE',
  });
}

export async function enableSite(id: number): Promise<SiteMessageResponse> {
  return request<SiteMessageResponse>(`/api/v1/sites/${id}/enable`, {
    method: 'POST',
  });
}

export async function disableSite(id: number): Promise<SiteMessageResponse> {
  return request<SiteMessageResponse>(`/api/v1/sites/${id}/disable`, {
    method: 'POST',
  });
}

export async function setForceHttps(id: number, forceHttps: boolean): Promise<Site> {
  return request<Site>(`/api/v1/sites/${id}/force-https`, {
    method: 'POST',
    body: JSON.stringify({ force_https: forceHttps }),
  });
}

// ==========================================
// Site Domains API
// ==========================================

export async function fetchSiteDomains(siteId: number): Promise<SiteDomain[]> {
  return request<SiteDomain[]>(`/api/v1/sites/${siteId}/domains`);
}

export async function addSiteDomain(siteId: number, data: AddDomainRequest): Promise<SiteDomain> {
  return request<SiteDomain>(`/api/v1/sites/${siteId}/domains`, {
    method: 'POST',
    body: JSON.stringify(data),
  });
}

export async function updateSiteDomain(
  siteId: number,
  domainId: number,
  data: UpdateDomainRequest
): Promise<SiteDomain> {
  return request<SiteDomain>(`/api/v1/sites/${siteId}/domains/${domainId}`, {
    method: 'PUT',
    body: JSON.stringify(data),
  });
}

export async function deleteSiteDomain(
  siteId: number,
  domainId: number
): Promise<SiteMessageResponse> {
  return request<SiteMessageResponse>(`/api/v1/sites/${siteId}/domains/${domainId}`, {
    method: 'DELETE',
  });
}

// ==========================================
// SSL & Certificate API
// ==========================================

export async function fetchSiteSsl(siteId: number): Promise<SslStatus> {
  return request<SslStatus>(`/api/v1/sites/${siteId}/ssl`);
}

export async function issueSiteSsl(
  siteId: number,
  data: IssueSslRequest
): Promise<SslActionResponse> {
  return request<SslActionResponse>(`/api/v1/sites/${siteId}/ssl/issue`, {
    method: 'POST',
    body: JSON.stringify(data),
  });
}

export async function generateSelfSignedSsl(siteId: number): Promise<SslActionResponse> {
  return request<SslActionResponse>(`/api/v1/sites/${siteId}/ssl/self-signed`, {
    method: 'POST',
  });
}

export async function uploadCustomSsl(
  siteId: number,
  data: CustomCertRequest
): Promise<SslActionResponse> {
  return request<SslActionResponse>(`/api/v1/sites/${siteId}/ssl/custom`, {
    method: 'POST',
    body: JSON.stringify(data),
  });
}

export async function renewSiteSsl(siteId: number): Promise<SslActionResponse> {
  return request<SslActionResponse>(`/api/v1/sites/${siteId}/ssl/renew`, {
    method: 'POST',
  });
}

export async function deleteSiteSsl(siteId: number): Promise<SslActionResponse> {
  return request<SslActionResponse>(`/api/v1/sites/${siteId}/ssl`, {
    method: 'DELETE',
  });
}

// ==========================================
// Site Logs API
// ==========================================

export async function fetchSiteLogs(
  siteId: number,
  logType: 'access' | 'error' | 'php_error' | 'php_slow' | string,
  lines?: number,
  query?: string
): Promise<SiteLogsResponse> {
  const params = new URLSearchParams();
  params.set('log_type', logType);
  if (lines) params.set('lines', lines.toString());
  if (query) params.set('query', query);
  return request<SiteLogsResponse>(`/api/v1/sites/${siteId}/logs?${params.toString()}`);
}

// ==========================================
// PHP Runtime API
// ==========================================

export async function fetchPhpVersions(): Promise<PhpRuntimeListApiResponse> {
  return request<PhpRuntimeListApiResponse>('/api/v1/php/versions');
}

export async function setupPhpRepo(): Promise<JobItem> {
  return request<JobItem>('/api/v1/php/repo/setup', {
    method: 'POST',
  });
}

export async function installPhpVersion(version: string): Promise<JobItem> {
  return request<JobItem>(`/api/v1/php/versions/${encodeURIComponent(version)}/install`, {
    method: 'POST',
  });
}

export async function removePhpVersion(version: string): Promise<JobItem> {
  return request<JobItem>(`/api/v1/php/versions/${encodeURIComponent(version)}/remove`, {
    method: 'POST',
  });
}

export async function fetchPhpExtensions(version: string): Promise<PhpExtensionListApiResponse> {
  return request<PhpExtensionListApiResponse>(`/api/v1/php/versions/${encodeURIComponent(version)}/extensions`);
}

export async function installPhpExtension(version: string, extension: string): Promise<JobItem> {
  return request<JobItem>(
    `/api/v1/php/versions/${encodeURIComponent(version)}/extensions/${encodeURIComponent(extension)}/install`,
    {
      method: 'POST',
    }
  );
}

export async function togglePhpExtension(
  version: string,
  extension: string,
  enabled: boolean
): Promise<PhpExtensionActionApiResponse> {
  return request<PhpExtensionActionApiResponse>(
    `/api/v1/php/versions/${encodeURIComponent(version)}/extensions/${encodeURIComponent(extension)}/toggle`,
    {
      method: 'POST',
      body: JSON.stringify({ enabled }),
    }
  );
}

export async function removePhpExtension(version: string, extension: string): Promise<JobItem> {
  return request<JobItem>(
    `/api/v1/php/versions/${encodeURIComponent(version)}/extensions/${encodeURIComponent(extension)}/remove`,
    {
      method: 'POST',
    }
  );
}

export async function fetchSitePhpExtensions(siteId: number): Promise<SitePhpExtensionsResponse> {
  return request<SitePhpExtensionsResponse>(`/api/v1/sites/${siteId}/php/extensions`);
}

export async function updateSitePhpExtensions(
  siteId: number,
  extensions: string[]
): Promise<SitePhpExtensionsResponse> {
  return request<SitePhpExtensionsResponse>(`/api/v1/sites/${siteId}/php/extensions`, {
    method: 'PUT',
    body: JSON.stringify({ extensions }),
  });
}

export async function switchSitePhpVersion(
  siteId: number,
  version: string
): Promise<SwitchPhpVersionResponse> {
  return request<SwitchPhpVersionResponse>(`/api/v1/sites/${siteId}/php/version`, {
    method: 'POST',
    body: JSON.stringify({ version }),
  });
}

export async function fetchSitePhpSettings(siteId: number): Promise<SitePhpSettingsResponse> {
  return request<SitePhpSettingsResponse>(`/api/v1/sites/${siteId}/php/settings`);
}

export async function updateSitePhpSettings(
  siteId: number,
  data: UpdateSitePhpSettingsInput
): Promise<SitePhpSettingsResponse> {
  return request<SitePhpSettingsResponse>(`/api/v1/sites/${siteId}/php/settings`, {
    method: 'PUT',
    body: JSON.stringify(data),
  });
}

export async function fetchSitePhpStatus(siteId: number): Promise<SitePhpStatusResponse> {
  return request<SitePhpStatusResponse>(`/api/v1/sites/${siteId}/php/status`);
}

export async function fetchSitePhpLogs(
  siteId: number,
  logType: 'error' | 'slow' = 'error',
  lines?: number,
  query?: string
): Promise<SitePhpLogsResponse> {
  const params = new URLSearchParams();
  params.set('type', logType);
  if (lines) params.set('lines', lines.toString());
  if (query) params.set('q', query);
  return request<SitePhpLogsResponse>(`/api/v1/sites/${siteId}/php/logs?${params.toString()}`);
}

// ==========================================
// File Manager API
// ==========================================

export async function fetchFileList(params: {
  path?: string;
  offset?: number;
  limit?: number;
  sort_by?: string;
  sort_order?: string;
  show_hidden?: boolean;
  search_query?: string;
  extension?: string;
}): Promise<FsListResponse> {
  const q = new URLSearchParams();
  if (params.path) q.set('path', params.path);
  if (params.offset !== undefined) q.set('offset', params.offset.toString());
  if (params.limit !== undefined) q.set('limit', params.limit.toString());
  if (params.sort_by) q.set('sort_by', params.sort_by);
  if (params.sort_order) q.set('sort_order', params.sort_order);
  if (params.show_hidden !== undefined) q.set('show_hidden', params.show_hidden ? 'true' : 'false');
  if (params.search_query) q.set('search_query', params.search_query);
  if (params.extension) q.set('extension', params.extension);

  const qs = q.toString() ? `?${q.toString()}` : '';
  return request<FsListResponse>(`/api/v1/files/list${qs}`);
}

export async function createFolder(
  path: string,
  createParents = true
): Promise<{ success: boolean; message: string }> {
  return request<{ success: boolean; message: string }>('/api/v1/files/mkdir', {
    method: 'POST',
    body: JSON.stringify({ path, create_parents: createParents }),
  });
}

export async function renameFile(
  from: string,
  to: string,
  overwrite = false
): Promise<{ success: boolean; message: string }> {
  return request<{ success: boolean; message: string }>('/api/v1/files/rename', {
    method: 'POST',
    body: JSON.stringify({ from, to, overwrite }),
  });
}

export async function moveFile(
  from: string,
  to: string,
  overwrite = false
): Promise<{ success: boolean; message: string }> {
  return request<{ success: boolean; message: string }>('/api/v1/files/move', {
    method: 'POST',
    body: JSON.stringify({ from, to, overwrite }),
  });
}

export async function copyFile(
  from: string,
  to: string,
  overwrite = false,
  asJob = false
): Promise<{ success: boolean; message: string }> {
  return request<{ success: boolean; message: string }>('/api/v1/files/copy', {
    method: 'POST',
    body: JSON.stringify({ from, to, overwrite, as_job: asJob }),
  });
}

export async function moveToTrash(
  path: string
): Promise<{ success: boolean; message: string }> {
  return request<{ success: boolean; message: string }>('/api/v1/files/trash', {
    method: 'POST',
    body: JSON.stringify({ path }),
  });
}

export async function fetchTrashList(): Promise<TrashListResponse> {
  return request<TrashListResponse>('/api/v1/files/trash');
}

export async function restoreTrash(
  trashId: string
): Promise<{ success: boolean; message: string }> {
  return request<{ success: boolean; message: string }>('/api/v1/files/trash/restore', {
    method: 'POST',
    body: JSON.stringify({ trash_id: trashId }),
  });
}

export async function emptyTrash(): Promise<{
  success: boolean;
  message: string;
  deleted_count: number;
  reclaimed_bytes: number;
}> {
  return request<{
    success: boolean;
    message: string;
    deleted_count: number;
    reclaimed_bytes: number;
  }>('/api/v1/files/trash/empty', {
    method: 'DELETE',
  });
}

export async function deleteFilePermanent(
  path: string
): Promise<{ success: boolean; message: string }> {
  return request<{ success: boolean; message: string }>('/api/v1/files/permanent', {
    method: 'DELETE',
    body: JSON.stringify({ path }),
  });
}

export async function compressArchive(
  sources: string[],
  targetArchive: string,
  format = 'zip'
): Promise<{ success: boolean; message: string; job_id?: string }> {
  return request<{ success: boolean; message: string; job_id?: string }>(
    '/api/v1/files/archive/compress',
    {
      method: 'POST',
      body: JSON.stringify({ sources, target_archive: targetArchive, format }),
    }
  );
}

export async function extractArchive(
  archivePath: string,
  targetDir = ''
): Promise<{ success: boolean; message: string; job_id?: string }> {
  return request<{ success: boolean; message: string; job_id?: string }>(
    '/api/v1/files/archive/extract',
    {
      method: 'POST',
      body: JSON.stringify({ archive_path: archivePath, target_dir: targetDir }),
    }
  );
}

export async function initUpload(
  path: string,
  totalSize: number,
  fileHash?: string
): Promise<{ upload_id: string; chunk_size: number }> {
  return request<{ upload_id: string; chunk_size: number }>('/api/v1/files/upload/init', {
    method: 'POST',
    body: JSON.stringify({ path, total_size: totalSize, file_hash: fileHash }),
  });
}

export async function uploadChunk(
  uploadId: string,
  offset: number,
  chunk: Blob
): Promise<{ success: boolean; current_offset: number; complete: boolean }> {
  const res = await fetch(`/api/v1/files/upload/${uploadId}/chunk?offset=${offset}`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/octet-stream',
    },
    body: chunk,
    credentials: 'include',
  });

  if (!res.ok) {
    const err = (await res.json().catch(() => ({}))) as { detail?: string; message?: string };
    throw new Error(err.detail || err.message || `Unggah gagal dengan status ${res.status}`);
  }

  return res.json() as Promise<{ success: boolean; current_offset: number; complete: boolean }>;
}

export async function createTerminalSession(
  payload: import('../types/terminal').CreateTerminalSessionPayload
): Promise<import('../types/terminal').TerminalSession> {
  return request<import('../types/terminal').TerminalSession>('/api/v1/terminal/sessions', {
    method: 'POST',
    body: JSON.stringify(payload),
  });
}

export async function fetchTerminalSessions(
  tenant?: string
): Promise<{ sessions: import('../types/terminal').TerminalSession[] }> {
  const query = tenant ? `?tenant=${encodeURIComponent(tenant)}` : '';
  return request<{ sessions: import('../types/terminal').TerminalSession[] }>(
    `/api/v1/terminal/sessions${query}`
  );
}

export async function getTerminalSession(
  sessionId: string
): Promise<import('../types/terminal').TerminalSession> {
  return request<import('../types/terminal').TerminalSession>(
    `/api/v1/terminal/sessions/${encodeURIComponent(sessionId)}`
  );
}

export async function closeTerminalSession(
  sessionId: string
): Promise<import('../types/terminal').TerminalCloseResponse> {
  return request<import('../types/terminal').TerminalCloseResponse>(
    `/api/v1/terminal/sessions/${encodeURIComponent(sessionId)}`,
    {
      method: 'DELETE',
    }
  );
}

export async function resizeTerminalSession(
  sessionId: string,
  cols: number,
  rows: number
): Promise<import('../types/terminal').TerminalResizeResponse> {
  return request<import('../types/terminal').TerminalResizeResponse>(
    `/api/v1/terminal/sessions/${encodeURIComponent(sessionId)}/resize`,
    {
      method: 'POST',
      body: JSON.stringify({ cols, rows }),
    }
  );
}

// SSH Key Management API (Phase 6 - P6-T5)
export async function fetchSshKeys(
  query?: { user_id?: number; tenant?: string }
): Promise<import('../types/api').SshKeyItem[]> {
  const params = new URLSearchParams();
  if (query?.user_id) params.set('user_id', String(query.user_id));
  if (query?.tenant) params.set('tenant', query.tenant);
  const q = params.toString() ? `?${params.toString()}` : '';
  return request<import('../types/api').SshKeyItem[]>(`/api/v1/ssh-keys${q}`);
}

export async function createSshKey(
  data: import('../types/api').CreateSshKeyRequest
): Promise<import('../types/api').SshKeyItem> {
  return request<import('../types/api').SshKeyItem>('/api/v1/ssh-keys', {
    method: 'POST',
    body: JSON.stringify(data),
  });
}

export async function deleteSshKey(
  id: number
): Promise<{ success: boolean; message: string }> {
  return request<{ success: boolean; message: string }>(
    `/api/v1/ssh-keys/${id}`,
    {
      method: 'DELETE',
    }
  );
}

export async function validateSshKey(
  publicKey: string
): Promise<import('../types/api').ValidateSshKeyResponse> {
  return request<import('../types/api').ValidateSshKeyResponse>(
    '/api/v1/ssh-keys/validate',
    {
      method: 'POST',
      body: JSON.stringify({ public_key: publicKey }),
    }
  );
}

// Shell Policy & SSH Drop-in Management (Phase 6 - P6-T6)
export async function fetchTenantShellPolicy(
  id: number
): Promise<import('../types/api').TenantShellPolicyResponse> {
  return request<import('../types/api').TenantShellPolicyResponse>(
    `/api/v1/tenants/${id}/shell-policy`
  );
}

export async function updateTenantShellPolicy(
  id: number,
  shellPolicy: string | null
): Promise<import('../types/api').TenantShellPolicyResponse> {
  return request<import('../types/api').TenantShellPolicyResponse>(
    `/api/v1/tenants/${id}/shell-policy`,
    {
      method: 'PUT',
      body: JSON.stringify({ shell_policy: shellPolicy }),
    }
  );
}

export async function applySshDropin(
  customContent?: string | null
): Promise<import('../types/api').SshDropinApplyResponse> {
  return request<import('../types/api').SshDropinApplyResponse>(
    '/api/v1/ssh/dropin/apply',
    {
      method: 'POST',
      body: JSON.stringify({ custom_content: customContent ?? null }),
    }
  );
}

// SSH Connection Command Info (Phase 6 - P6-T7)
export async function fetchSshConnectionInfo(
  query?: import('../types/api').SshConnectionInfoQuery
): Promise<import('../types/api').SshConnectionInfoResponse> {
  const params = new URLSearchParams();
  if (query?.site_id !== undefined) params.set('site_id', query.site_id.toString());
  if (query?.path !== undefined) params.set('path', query.path);
  if (query?.tenant !== undefined) params.set('tenant', query.tenant);
  if (query?.port !== undefined) params.set('port', query.port.toString());
  if (query?.host !== undefined) params.set('host', query.host);

  const qs = params.toString() ? `?${params.toString()}` : '';
  return request<import('../types/api').SshConnectionInfoResponse>(
    `/api/v1/ssh/connection-info${qs}`
  );
}

// ==========================================
// Node.js Runtime API (Phase 7 - P7-T1)
// ==========================================

export async function fetchNodeVersions(
  mirrorUrl?: string,
  refresh?: boolean
): Promise<import('../types/api').NodeRuntimeListApiResponse> {
  const params = new URLSearchParams();
  if (mirrorUrl) params.set('mirror_url', mirrorUrl);
  if (refresh) params.set('refresh', 'true');
  const qs = params.toString() ? `?${params.toString()}` : '';
  return request<import('../types/api').NodeRuntimeListApiResponse>(
    `/api/v1/runtimes/node/versions${qs}`
  );
}

export async function installNodeVersion(
  payload: import('../types/api').NodeRuntimeInstallRequest
): Promise<import('../types/api').NodeRuntimeActionResponse> {
  return request<import('../types/api').NodeRuntimeActionResponse>(
    '/api/v1/runtimes/node/install',
    {
      method: 'POST',
      body: JSON.stringify(payload),
    }
  );
}

export async function removeNodeVersion(
  version: string
): Promise<import('../types/api').NodeRuntimeActionResponse> {
  return request<import('../types/api').NodeRuntimeActionResponse>(
    `/api/v1/runtimes/node/${encodeURIComponent(version)}`,
    {
      method: 'DELETE',
    }
  );
}

export async function enableNodeCorepack(
  version: string
): Promise<import('../types/api').NodeRuntimeActionResponse> {
  return request<import('../types/api').NodeRuntimeActionResponse>(
    `/api/v1/runtimes/node/${encodeURIComponent(version)}/corepack`,
    {
      method: 'POST',
    }
  );
}

// Node.js Site & Process Management Client APIs (Phase 7 - P7-T2)
export async function getNodeSiteStatus(
  siteId: number
): Promise<import('../types/api').NodeSiteStatusApiResponse> {
  return request<import('../types/api').NodeSiteStatusApiResponse>(
    `/api/v1/sites/${siteId}/node/status`
  );
}

export async function postNodeSiteAction(
  siteId: number,
  payload: import('../types/api').NodeSiteActionApiRequest
): Promise<{ success: boolean; message: string }> {
  return request<{ success: boolean; message: string }>(
    `/api/v1/sites/${siteId}/node/action`,
    {
      method: 'POST',
      body: JSON.stringify(payload),
    }
  );
}

export async function runNodeSiteJob(
  siteId: number,
  payload: import('../types/api').NodeSiteJobApiRequest
): Promise<import('../types/api').NodeSiteJobApiResponse> {
  return request<import('../types/api').NodeSiteJobApiResponse>(
    `/api/v1/sites/${siteId}/node/job`,
    {
      method: 'POST',
      body: JSON.stringify(payload),
    }
  );
}

export async function getNodeSiteSettings(
  siteId: number
): Promise<import('../types/api').NodeSiteSettingsDto> {
  return request<import('../types/api').NodeSiteSettingsDto>(
    `/api/v1/sites/${siteId}/node/settings`
  );
}

export async function updateNodeSiteSettings(
  siteId: number,
  payload: import('../types/api').UpdateNodeSiteSettingsRequest
): Promise<import('../types/api').NodeSiteSettingsDto> {
  return request<import('../types/api').NodeSiteSettingsDto>(
    `/api/v1/sites/${siteId}/node/settings`,
    {
      method: 'PUT',
      body: JSON.stringify(payload),
    }
  );
}

// Python Runtime & Site Management Client APIs (Phase 7 - P7-T3)
export async function getPythonVersions(
  refresh?: boolean
): Promise<import('../types/api').PythonVersionSummaryDto[]> {
  const qs = refresh ? '?refresh=true' : '';
  const res = await request<import('../types/api').PythonRuntimeListApiResponse>(
    `/api/v1/runtimes/python${qs}`
  );
  return res.versions;
}

export async function installPythonVersion(
  payload: { version: string }
): Promise<{ success: boolean; message: string; version: string }> {
  return request<{ success: boolean; message: string; version: string }>(
    '/api/v1/runtimes/python',
    {
      method: 'POST',
      body: JSON.stringify(payload),
    }
  );
}

export async function removePythonVersion(
  version: string
): Promise<{ success: boolean; message: string; version: string }> {
  return request<{ success: boolean; message: string; version: string }>(
    `/api/v1/runtimes/python/${encodeURIComponent(version)}`,
    {
      method: 'DELETE',
    }
  );
}

export async function getPythonSiteStatus(
  siteId: number
): Promise<import('../types/api').PythonSiteStatusApiResponse> {
  return request<import('../types/api').PythonSiteStatusApiResponse>(
    `/api/v1/sites/${siteId}/python/status`
  );
}

export async function postPythonSiteAction(
  siteId: number,
  payload: import('../types/api').PythonSiteActionApiRequest
): Promise<{ success: boolean; message: string }> {
  return request<{ success: boolean; message: string }>(
    `/api/v1/sites/${siteId}/python/action`,
    {
      method: 'POST',
      body: JSON.stringify(payload),
    }
  );
}

export async function runPythonSiteJob(
  siteId: number,
  payload: import('../types/api').PythonSiteJobApiRequest
): Promise<import('../types/api').PythonSiteJobApiResponse> {
  return request<import('../types/api').PythonSiteJobApiResponse>(
    `/api/v1/sites/${siteId}/python/job`,
    {
      method: 'POST',
      body: JSON.stringify(payload),
    }
  );
}

export async function getPythonSiteSettings(
  siteId: number
): Promise<import('../types/api').PythonSiteSettingsDto> {
  return request<import('../types/api').PythonSiteSettingsDto>(
    `/api/v1/sites/${siteId}/python/settings`
  );
}

export async function updatePythonSiteSettings(
  siteId: number,
  payload: import('../types/api').UpdatePythonSiteSettingsRequest
): Promise<import('../types/api').PythonSiteSettingsDto> {
  return request<import('../types/api').PythonSiteSettingsDto>(
    `/api/v1/sites/${siteId}/python/settings`,
    {
      method: 'PUT',
      body: JSON.stringify(payload),
    }
  );
}

// Reverse Proxy API (Phase 7 - P7-T4)
export async function getProxySiteSettings(
  siteId: number
): Promise<import('../types/api').ProxySiteSettingsDto> {
  return request<import('../types/api').ProxySiteSettingsDto>(
    `/api/v1/sites/${siteId}/proxy/settings`
  );
}

export async function updateProxySiteSettings(
  siteId: number,
  payload: import('../types/api').UpdateProxySiteSettingsRequest
): Promise<import('../types/api').ProxySiteSettingsDto> {
  return request<import('../types/api').ProxySiteSettingsDto>(
    `/api/v1/sites/${siteId}/proxy/settings`,
    {
      method: 'PUT',
      body: JSON.stringify(payload),
    }
  );
}

export async function probeProxyUpstream(
  siteId: number,
  upstreamUrl?: string
): Promise<import('../types/api').ProxyUpstreamProbeResponse> {
  const url = upstreamUrl
    ? `/api/v1/sites/${siteId}/proxy/probe?upstream_url=${encodeURIComponent(upstreamUrl)}`
    : `/api/v1/sites/${siteId}/proxy/probe`;
  return request<import('../types/api').ProxyUpstreamProbeResponse>(url);
}

// Unified Process Management API (Phase 7 - P7-T5)
export async function fetchProcesses(): Promise<import('../types/api').ProcessListResponse> {
  return request<import('../types/api').ProcessListResponse>('/api/v1/processes');
}


