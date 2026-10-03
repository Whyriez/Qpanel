import { HealthResponse, SystemInfoResponse } from '../types/api';

const API_BASE = '';

export async function fetchHealth(): Promise<HealthResponse> {
  const res = await fetch(`${API_BASE}/healthz`);
  if (!res.ok) {
    throw new Error(`Health check failed: ${res.statusText}`);
  }
  return res.json();
}

export async function fetchSystemInfo(): Promise<SystemInfoResponse> {
  const res = await fetch(`${API_BASE}/api/v1/system/info`);
  if (!res.ok) {
    throw new Error(`Failed to fetch system info: ${res.statusText}`);
  }
  return res.json();
}
