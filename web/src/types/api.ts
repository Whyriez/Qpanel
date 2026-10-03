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
