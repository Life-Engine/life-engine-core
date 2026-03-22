import type { CoreConfig, SystemInfo, PluginInfo } from "../types/config";

/** Error thrown by API client methods. */
export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

/** Base URL for API requests (empty string uses the Vite dev proxy). */
const BASE = "";

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${BASE}${path}`, init);
  const body = await res.json();

  if (!res.ok) {
    throw new ApiError(res.status, body.error ?? res.statusText);
  }

  return body.data as T;
}

/** Fetch the current (redacted) configuration. */
export async function getConfig(): Promise<CoreConfig> {
  return request<CoreConfig>("/api/system/config");
}

/** Send a partial config update and return the merged result. */
export async function updateConfig(
  partial: Partial<CoreConfig>,
): Promise<CoreConfig> {
  return request<CoreConfig>("/api/system/config", {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(partial),
  });
}

/** Fetch system information (version, uptime, etc.). */
export async function getSystemInfo(): Promise<SystemInfo> {
  return request<SystemInfo>("/api/system/info");
}

/** Fetch the list of loaded plugins. */
export async function getPlugins(): Promise<PluginInfo[]> {
  return request<PluginInfo[]>("/api/system/plugins");
}

/** Perform a health check. Returns true if the server is healthy. */
export async function healthCheck(): Promise<boolean> {
  try {
    const res = await fetch(`${BASE}/api/system/health`);
    return res.ok;
  } catch {
    return false;
  }
}
