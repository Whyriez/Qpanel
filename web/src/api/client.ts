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
} from '../types/api';

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

