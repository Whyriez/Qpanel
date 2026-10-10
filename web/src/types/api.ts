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
  shell_policy?: string | null;
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
  kind?: 'static' | 'php' | 'nodejs' | 'python' | 'proxy' | string;
  runtime?: string;
  version?: string;
  port?: number;
  app_dir?: string;
  entrypoint?: string;
  entrypoint_type?: string;
  auto_restart?: boolean;
  env_vars?: Record<string, string>;
  framework?: string;
  server_type?: string;
  workers?: number;
  venv_dir?: string;
  static_url?: string;
  static_dir?: string;
  media_url?: string;
  media_dir?: string;
  upstream_url?: string;
  preserve_host?: boolean;
  websocket_support?: boolean;
  proxy_connect_timeout?: string;
  proxy_send_timeout?: string;
  proxy_read_timeout?: string;
  proxy_buffering?: boolean;
  custom_request_headers?: ProxyHeaderConfig[];
  custom_response_headers?: ProxyHeaderConfig[];
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
  log_type: 'access' | 'error' | 'php_error' | 'php_slow' | string;
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

// PHP Settings Types
export interface PhpSettings {
  memory_limit_mb: number;
  upload_max_filesize_mb: number;
  post_max_size_mb: number;
  max_execution_time: number;
  max_input_vars: number;
  display_errors: boolean;
  date_timezone: string;
  opcache_enable: boolean;
  opcache_memory_consumption_mb: number;
  opcache_validate_timestamps: boolean;
  opcache_revalidate_freq: number;
}

export interface PhpSettingsLimits {
  memory_limit_mb: number;
  upload_limit_mb: number;
  max_execution_time: number;
  max_input_vars: number;
  opcache_memory_consumption_mb: number;
}

export interface SitePhpSettingsResponse {
  site_id: number;
  php_version: string;
  settings: PhpSettings;
  limits: PhpSettingsLimits;
  admin_ini?: string | null;
  is_admin: boolean;
}

export interface UpdateSitePhpSettingsInput {
  settings: PhpSettings;
  admin_ini?: string | null;
}

// PHP-FPM Status & Metrics Types
export interface PhpFpmStatusMetrics {
  pool: string;
  process_manager: string;
  start_time: number;
  start_since: number;
  accepted_conn: number;
  listen_queue: number;
  max_listen_queue: number;
  listen_queue_len: number;
  idle_processes: number;
  active_processes: number;
  total_processes: number;
  max_active_processes: number;
  max_children_reached: number;
  slow_requests: number;
}

export interface SitePhpStatusResponse {
  site_id: number;
  primary_domain: string;
  php_version: string;
  service_name: string;
  is_active: boolean;
  socket_exists: boolean;
  metrics: PhpFpmStatusMetrics | null;
}

export interface SitePhpLogsResponse {
  site_id: number;
  primary_domain: string;
  php_version: string;
  log_type: string;
  lines: string[];
  total_matched: number;
}

// SSH Key Types (Phase 6 - P6-T5)
export interface SshKeyItem {
  id: number;
  user_id: number;
  name: string;
  key_type: string;
  fingerprint_sha256: string;
  fingerprint_md5?: string | null;
  public_key: string;
  created_at: number;
}

export interface CreateSshKeyRequest {
  name: string;
  public_key: string;
}

export interface ValidateSshKeyRequest {
  public_key: string;
}

export interface ValidateSshKeyResponse {
  valid: boolean;
  key_type?: string | null;
  fingerprint_sha256?: string | null;
  fingerprint_md5?: string | null;
  comment?: string | null;
  bit_length?: number | null;
  error?: string | null;
}

export interface TenantShellPolicyResponse {
  user_id: number;
  username: string;
  shell_policy: string | null;
  effective_policy: string;
  inherited_from_plan: boolean;
  plan_policy: string | null;
}

export interface UpdateTenantShellPolicyRequest {
  shell_policy: string | null;
}

export interface SshDropinApplyRequest {
  custom_content?: string | null;
}

export interface SshDropinApplyResponse {
  success: boolean;
  path: string;
  validated: boolean;
  reloaded: boolean;
  message: string;
}

// SSH & SFTP Connection Details Types (Phase 6 - P6-T7)
export interface SshConnectionInfoQuery {
  site_id?: number;
  path?: string;
  tenant?: string;
  port?: number;
  host?: string;
}

export interface SshConnectionInfoResponse {
  host: string;
  port: number;
  username: string;
  user_id: number;
  home_dir: string;
  active_path: string;
  relative_path: string;
  shell_policy: 'full' | 'sftp' | 'none' | string;
  can_shell: boolean;
  can_sftp: boolean;
  has_ssh_keys: boolean;
  ssh_keys_count: number;
  ssh_command: string | null;
  sftp_command: string | null;
  sftp_url: string | null;
  ssh_config_snippet: string;
  site_domain: string | null;
}

// Node.js Runtime Types (Phase 7 - P7-T1)
export interface NodeVersionItem {
  version: string;
  raw_version: string;
  is_installed: boolean;
  installed_path: string | null;
  is_lts: boolean;
  lts_codename: string | null;
  release_date: string | null;
  npm_version: string | null;
  corepack_enabled: boolean;
  used_by_sites_count: number;
  used_by_domains: string[];
  can_delete: boolean;
}

export interface NodeRuntimeListApiResponse {
  mirror_url: string;
  versions: NodeVersionItem[];
  installed_count: number;
  is_offline_fallback: boolean;
}

export interface NodeRuntimeInstallRequest {
  version: string;
  mirror_url?: string;
  offline_tarball_path?: string;
  offline_checksum?: string;
  enable_corepack?: boolean;
}

export interface NodeRuntimeActionResponse {
  version: string;
  success: boolean;
  path: string;
  message: string;
}

// Node.js Site & Process Management Types (Phase 7 - P7-T2)
export interface NodeSiteStatusApiResponse {
  site_id: string;
  service_name: string;
  active: boolean;
  sub_state: string;
  pid: number | null;
  memory_bytes: number | null;
  cpu_usage_usec: number | null;
  uptime_secs: number | null;
  healthy: boolean;
  port: number;
  probe_status_code: number | null;
  probe_message: string | null;
}

export interface NodeSiteActionApiRequest {
  action: 'start' | 'stop' | 'restart' | 'redeploy' | string;
}

export interface NodeSiteJobApiRequest {
  job_type: 'npm_install' | 'npm_ci' | 'npm_build' | 'pnpm_install' | 'yarn_install' | 'custom' | string;
  script_name?: string;
  timeout_secs?: number;
}

export interface NodeSiteJobApiResponse {
  site_id: number;
  command: string;
  exit_code: number;
  success: boolean;
  stdout: string;
  stderr: string;
  duration_ms: number;
}

export interface NodeSiteSettingsDto {
  site_id: number;
  node_version: string;
  port: number;
  app_dir: string;
  entrypoint_type: 'script' | 'npm_start' | 'npm_custom' | string;
  entrypoint_value: string | null;
  auto_restart: boolean;
  env_vars: Record<string, string>;
}

export interface UpdateNodeSiteSettingsRequest {
  node_version?: string;
  port?: number;
  app_dir?: string;
  entrypoint_type?: string;
  entrypoint_value?: string;
  auto_restart?: boolean;
  env_vars?: Record<string, string>;
}

// Python Site & Process Management Types (Phase 7 - P7-T3)
export interface PythonRuntimeListApiResponse {
  versions: PythonVersionSummaryDto[];
}

export interface PythonVersionSummaryDto {
  version: string;
  major_minor: string;
  release_date: string | null;
  is_default: boolean;
  is_lts_or_stable: boolean;
  description: string;
  is_installed: boolean;
  install_path: string | null;
  bin_path: string | null;
}

export interface PythonSiteStatusApiResponse {
  site_id: string;
  service_name: string;
  active: boolean;
  sub_state: string;
  pid: number | null;
  memory_bytes: number | null;
  cpu_usage_usec: number | null;
  uptime_secs: number | null;
  socket_path: string;
  socket_exists: boolean;
  healthy: boolean;
  probe_message: string | null;
}

export interface PythonSiteActionApiRequest {
  action: 'start' | 'stop' | 'restart' | 'redeploy' | string;
}

export interface PythonSiteJobApiRequest {
  job_type: 'uv_venv' | 'uv_pip_install' | 'uv_pip_pyproject' | 'custom' | string;
  req_file?: string;
  script_name?: string;
  timeout_secs?: number;
}

export interface PythonSiteJobApiResponse {
  site_id: number;
  command: string;
  exit_code: number;
  success: boolean;
  stdout: string;
  stderr: string;
  duration_ms: number;
}

export interface PythonSiteSettingsDto {
  site_id: number;
  python_version: string;
  framework_preset: 'django' | 'flask' | 'fastapi' | 'generic' | string;
  server_type: 'gunicorn_wsgi' | 'uvicorn_asgi' | 'gunicorn_uvicorn_worker' | string;
  entrypoint_type: 'wsgi' | 'asgi' | 'custom' | string;
  entrypoint_value: string;
  workers: number;
  app_dir: string;
  venv_dir: string;
  socket_path: string;
  static_url: string | null;
  static_dir: string | null;
  media_url: string | null;
  media_dir: string | null;
  auto_restart: boolean;
  env_vars: Record<string, string>;
}

export interface UpdatePythonSiteSettingsRequest {
  python_version?: string;
  framework_preset?: string;
  server_type?: string;
  entrypoint_type?: string;
  entrypoint_value?: string;
  workers?: number;
  app_dir?: string;
  venv_dir?: string;
  static_url?: string;
  static_dir?: string;
  media_url?: string;
  media_dir?: string;
  auto_restart?: boolean;
  env_vars?: Record<string, string>;
}

// Reverse Proxy Types (Phase 7 - P7-T4)
export interface ProxyHeaderConfig {
  name: string;
  value: string;
}

export interface ProxySiteSettingsDto {
  site_id: number;
  upstream_url: string;
  preserve_host: boolean;
  websocket_support: boolean;
  proxy_connect_timeout: string | null;
  proxy_send_timeout: string | null;
  proxy_read_timeout: string | null;
  proxy_buffering: boolean | null;
  custom_request_headers: ProxyHeaderConfig[];
  custom_response_headers: ProxyHeaderConfig[];
  allow_external_upstream: boolean;
}

export interface UpdateProxySiteSettingsRequest {
  upstream_url?: string;
  preserve_host?: boolean;
  websocket_support?: boolean;
  proxy_connect_timeout?: string;
  proxy_send_timeout?: string;
  proxy_read_timeout?: string;
  proxy_buffering?: boolean;
  custom_request_headers?: ProxyHeaderConfig[];
  custom_response_headers?: ProxyHeaderConfig[];
}

export interface ProxyUpstreamProbeResponse {
  healthy: boolean;
  status_code: number | null;
  latency_ms: number;
  message: string;
}

// Unified Process Management Types (Phase 7 - P7-T5)
export interface ProcessItemDto {
  site_id: number;
  primary_domain: string;
  runtime: string;
  version?: string | null;
  service_name: string;
  active: boolean;
  sub_state: string;
  pid?: number | null;
  memory_bytes?: number | null;
  cpu_usage_usec?: number | null;
  uptime_secs?: number | null;
  healthy: boolean;
  port_or_socket?: string | null;
  probe_message?: string | null;
}

export interface ProcessListResponse {
  processes: ProcessItemDto[];
  total_active: number;
  total_memory_bytes: number;
}

// Environment Variables Types (Phase 8 - P8-T1)
export interface EnvVariableItem {
  key: string;
  value: string;
  is_masked: boolean;
}

export interface SiteEnvResponse {
  site_id: number;
  variables: EnvVariableItem[];
  restart_required: boolean;
}

export interface UpdateSiteEnvRequest {
  variables: Record<string, string>;
}

export interface ImportDotEnvRequest {
  content: string;
  merge?: boolean;
}

export interface SiteEnvActionResponse {
  success: boolean;
  message: string;
  restart_required: boolean;
}

// Cron Jobs Types (Phase 8 - P8-T2)
export interface CronJob {
  id: number;
  site_id: number;
  name: string;
  command: string;
  schedule: string;
  systemd_calendar: string;
  human_description: string;
  human_description_en: string;
  description?: string | null;
  work_dir?: string | null;
  timeout_secs: number;
  enabled: boolean;
  status: 'idle' | 'running' | 'success' | 'failed' | string;
  last_run_at?: number | null;
  last_duration_ms?: number | null;
  last_exit_code?: number | null;
  created_at: number;
  updated_at: number;
}

export interface CronQuota {
  used: number;
  limit: number;
}

export interface CronJobListResponse {
  jobs: CronJob[];
  quota: CronQuota;
}

export interface CreateCronJobRequest {
  name: string;
  command: string;
  schedule: string;
  description?: string;
  work_dir?: string;
  timeout_secs?: number;
  enabled?: boolean;
}

export interface UpdateCronJobRequest {
  name?: string;
  command?: string;
  schedule?: string;
  description?: string;
  work_dir?: string;
  timeout_secs?: number;
  enabled?: boolean;
}

export interface CronRun {
  id: number;
  cron_job_id: number;
  status: 'running' | 'success' | 'failed' | string;
  exit_code?: number | null;
  stdout?: string | null;
  stderr?: string | null;
  duration_ms?: number | null;
  started_at: number;
  finished_at?: number | null;
}

export interface ValidateCronResponse {
  valid: boolean;
  systemd_calendar?: string | null;
  human_id?: string | null;
  human_en?: string | null;
  error?: string | null;
}

