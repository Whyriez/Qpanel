export interface HealthResponse {
  status: string;
  timestamp: number;
}

export interface SystemInfoResponse {
  api_version: string;
  agent_version: string | null;
  proto_version: number;
  os: string;
  arch: string;
  agent_connected: boolean;
  environment: string;
}

export type ThemeMode = 'light' | 'dark' | 'system';

export interface User {
  id: number;
  username: string;
  role: 'admin' | 'user' | 'reseller' | string;
  email: string | null;
  totp_enabled: boolean;
}

export interface LoginResponse {
  message?: string;
  user?: User;
  requires_2fa?: boolean;
  csrf_token?: string;
}

export interface CsrfResponse {
  csrf_token: string;
}

export interface TwoFactorSetupResponse {
  secret: string;
  uri: string;
  qr_code_base64: string;
}

export interface TwoFactorStatusResponse {
  enabled: boolean;
  remaining_recovery_codes: number;
}

export interface TwoFactorEnableResponse {
  message: string;
  recovery_codes: string[];
}

export interface AuditLogItem {
  id: number;
  user_id: number | null;
  actor_username: string;
  action: string;
  target_resource: string;
  details_json: string | null;
  ip_address: string | null;
  status: string;
  created_at: number;
}

export interface AuditLogListResponse {
  items: AuditLogItem[];
  total: number;
  page: number;
  per_page: number;
  total_pages: number;
}

export interface AuditLogQuery {
  page?: number;
  per_page?: number;
  user_id?: number;
  actor?: string;
  action?: string;
  target?: string;
  status?: string;
  from?: number;
  to?: number;
}

export type JobStatus = 'queued' | 'running' | 'succeeded' | 'failed' | 'cancelled';

export interface JobItem {
  id: string;
  user_id: number | null;
  name: string;
  status: JobStatus;
  progress: number;
  payload_json: string;
  result_json: string | null;
  error_message: string | null;
  retry_count: number;
  max_retries: number;
  created_at: number;
  updated_at: number;
}

export interface JobListResponse {
  items: JobItem[];
  total: number;
  page: number;
  per_page: number;
  total_pages: number;
}

export interface JobProgressEvent {
  job_id: string;
  status: JobStatus;
  progress: number;
  message: string | null;
  timestamp: number;
}
