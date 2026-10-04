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

// Site & Domain Types
export type RedirectMode = 'none' | 'www_to_non_www' | 'non_www_to_www' | 'custom_url';

export interface SiteDomain {
  id: number;
  site_id: number;
  name: string;
  unicode_name: string;
  is_primary: boolean;
  redirect_mode: RedirectMode | string;
  redirect_target: string | null;
  status: string;
  created_at: number;
  updated_at: number;
}

export interface Site {
  id: number;
  user_id: number;
  kind: 'static' | 'php' | 'proxy' | string;
  primary_domain: string;
  docroot: string;
  status: 'active' | 'disabled' | 'suspended' | string;
  spa_fallback: boolean;
  force_https: boolean;
  domains: SiteDomain[];
  runtime?: string | null;
  version?: string | null;
  preset?: string | null;
  created_at: number;
  updated_at: number;
}

export interface CreateSiteRequest {
  domain: string;
  user_id?: number;
  spa_fallback?: boolean;
  force_https?: boolean;
  docroot?: string;
}

export interface AddDomainRequest {
  domain: string;
  redirect_mode?: RedirectMode | string;
  redirect_target?: string;
  is_primary?: boolean;
}

export interface UpdateDomainRequest {
  redirect_mode?: RedirectMode | string;
  redirect_target?: string;
  is_primary?: boolean;
}

export interface SiteMessageResponse {
  message: string;
  site_id: number;
  status: string;
}

// SSL & Certificate Types
export interface CertificateDetail {
  id: number;
  primary_domain: string;
  san_domains: string[];
  issuer: string;
  not_before: number;
  not_after: number;
  days_remaining: number;
  auto_renew: boolean;
  ocsp_stapling: boolean;
  last_renew_attempt: number | null;
  renew_error: string | null;
  status: string;
}

export interface SslStatus {
  has_ssl: boolean;
  certificate: CertificateDetail | null;
  force_https: boolean;
}

export interface IssueSslRequest {
  email?: string;
  staging?: boolean;
  fallback_self_signed?: boolean;
  ocsp_stapling?: boolean;
}

export interface CustomCertRequest {
  fullchain_pem: string;
  privkey_pem: string;
  auto_renew?: boolean;
}

export interface SslActionResponse {
  message: string;
  certificate?: CertificateDetail | null;
}

// Site Logs Types
export interface SiteLogsResponse {
  site_id: number;
  log_type: 'access' | 'error' | string;
  lines: string[];
  total_returned: number;
  file_path: string;
  file_size_bytes: number;
}

export interface PhpVersionStatusItem {
  version: string;
  installed: boolean;
  fpm_binary_exists: boolean;
  cli_binary_exists: boolean;
  service_masked: boolean;
  status: string;
  fpm_binary_path: string;
  cli_binary_path: string;
  service_name: string;
}

export interface PhpRuntimeListApiResponse {
  ppa_configured: boolean;
  versions: PhpVersionStatusItem[];
}

export interface PhpExtensionItem {
  name: string;
  version: string;
  package_name: string;
  installed: boolean;
  enabled: boolean;
  is_base: boolean;
  category: string;
  description: string;
}

export interface PhpExtensionListApiResponse {
  version: string;
  extensions: PhpExtensionItem[];
}

export interface ToggleExtensionRequest {
  enabled: boolean;
}

export interface PhpExtensionActionApiResponse {
  version: string;
  extension: string;
  success: boolean;
  message: string;
}

export interface SitePhpExtensionsResponse {
  site_id: number;
  php_version: string;
  extensions: string[];
  available_extensions: string[];
}

export interface UpdateSitePhpExtensionsInput {
  extensions: string[];
}

export interface SwitchPhpVersionRequest {
  version: string;
}

export interface SwitchPhpVersionResponse {
  message: string;
  site_id: number;
  old_version: string;
  new_version: string;
  vhost_reloaded: boolean;
}



