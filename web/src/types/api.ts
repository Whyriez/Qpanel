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
  impersonated_by?: string | null;
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

export interface Plan {
  id: number;
  name: string;
  description: string | null;
  cpu_quota_percent: number;
  memory_limit_bytes: number;
  memory_high_bytes: number | null;
  tasks_max: number;
  io_weight: number;
  disk_limit_bytes: number;
  inode_limit: number;
  max_sites: number;
  max_domains: number;
  max_databases: number;
  max_cron_jobs: number;
  max_terminal_sessions: number;
  max_upload_bytes: number;
  shell_policy: string;
  allow_external_upstream: boolean;
  created_at: number;
  updated_at: number;
}

export interface CreatePlanRequest {
  name: string;
  description?: string;
  cpu_quota_percent?: number;
  memory_limit_bytes?: number;
  memory_high_bytes?: number;
  tasks_max?: number;
  io_weight?: number;
  disk_limit_bytes?: number;
  inode_limit?: number;
  max_sites?: number;
  max_domains?: number;
  max_databases?: number;
  max_cron_jobs?: number;
  max_terminal_sessions?: number;
  max_upload_bytes?: number;
  shell_policy?: string;
  allow_external_upstream?: boolean;
}

export interface UpdatePlanRequest {
  name?: string;
  description?: string;
  cpu_quota_percent?: number;
  memory_limit_bytes?: number;
  memory_high_bytes?: number;
  tasks_max?: number;
  io_weight?: number;
  disk_limit_bytes?: number;
  inode_limit?: number;
  max_sites?: number;
  max_domains?: number;
  max_databases?: number;
  max_cron_jobs?: number;
  max_terminal_sessions?: number;
  max_upload_bytes?: number;
  shell_policy?: string;
  allow_external_upstream?: boolean;
}

export interface PlanListResponse {
  plans: Plan[];
}

export interface PlanResponse {
  plan: Plan;
}

export interface TenantListItem {
  id: number;
  username: string;
  email: string | null;
  status: string;
  linux_uid: number | null;
  linux_username: string | null;
  plan_id: number | null;
  plan_name: string | null;
  created_at: number;
  updated_at: number;
}

export interface TenantListResponse {
  tenants: TenantListItem[];
}

export interface TenantDetailResponse {
  tenant: TenantListItem;
  plan: Plan | null;
}

export interface CreateTenantRequest {
  username_slug: string;
  email?: string;
  password: string;
  plan_id: number;
}

export interface SliceResourceMetrics {
  uid: number;
  slice_name: string;
  exists: boolean;
  cpu_usage_usec: number;
  memory_current_bytes: number;
  memory_peak_bytes?: number | null;
  pids_current: number;
  pids_peak?: number | null;
  io_read_bytes: number;
  io_write_bytes: number;
}

export interface QuotaMetrics {
  uid: number;
  username: string;
  bytes_used: number;
  bytes_limit: number;
  inodes_used: number;
  inodes_limit: number;
  backend: string;
}

export interface TenantResourceUsage {
  uid: number;
  linux_username: string;
  metrics: SliceResourceMetrics;
  quota?: QuotaMetrics | null;
}

export interface PrerequisiteCheckItem {
  name: string;
  ok: boolean;
  message: string;
  details?: string | null;
}

export interface HardeningStatus {
  cgroup_v2: PrerequisiteCheckItem;
  acl: PrerequisiteCheckItem;
  quota: PrerequisiteCheckItem;
  hidepid: PrerequisiteCheckItem;
  tenant_group: PrerequisiteCheckItem;
  overall_ok: boolean;
}

export interface HardeningApplyRequest {
  enable_hidepid: boolean;
  ensure_tenant_group: boolean;
}

export interface HardeningApplyResponse {
  hidepid_applied: boolean;
  tenant_group_created: boolean;
  message: string;
}

