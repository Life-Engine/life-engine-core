import { useCallback, useEffect, useState } from "react";
import { getSystemInfo, healthCheck } from "../api/client";
import type { SystemInfo } from "../types/config";

function formatUptime(seconds: number): string {
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = Math.floor(seconds % 60);

  const parts: string[] = [];
  if (days > 0) parts.push(`${days}d`);
  if (hours > 0) parts.push(`${hours}h`);
  if (minutes > 0) parts.push(`${minutes}m`);
  parts.push(`${secs}s`);
  return parts.join(" ");
}

export default function SystemPage() {
  const [info, setInfo] = useState<SystemInfo | null>(null);
  const [healthy, setHealthy] = useState<boolean | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    setError(null);
    Promise.all([getSystemInfo(), healthCheck()])
      .then(([sysInfo, isHealthy]) => {
        setInfo(sysInfo);
        setHealthy(isHealthy);
      })
      .catch((err) => setError(err.message))
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    load();
    const interval = setInterval(load, 30_000);
    return () => clearInterval(interval);
  }, [load]);

  if (loading) {
    return (
      <div className="flex items-center gap-2 text-gray-500">
        <svg className="h-5 w-5 animate-spin" viewBox="0 0 24 24" fill="none">
          <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
          <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v4a4 4 0 00-4 4H4z" />
        </svg>
        Loading system information…
      </div>
    );
  }

  if (error) {
    return (
      <div className="rounded-lg border border-red-200 bg-red-50 p-4 text-sm text-red-700">
        Failed to load system information: {error}
      </div>
    );
  }

  if (!info) return null;

  return (
    <div>
      <div className="mb-6">
        <h1 className="text-2xl font-semibold text-gray-900">System</h1>
        <p className="mt-1 text-sm text-gray-600">
          System health, version, and uptime information. Auto-refreshes every 30 seconds.
        </p>
      </div>

      <div className="space-y-6">
        <div className="rounded-lg border border-gray-200 bg-white">
          <div className="px-6 py-4">
            <h2 className="text-lg font-medium text-gray-900">Health Status</h2>
          </div>
          <div className="border-t border-gray-200 px-6 py-4">
            <div className="flex items-center gap-3">
              <span
                className={`inline-flex h-3 w-3 rounded-full ${
                  healthy ? "bg-green-500" : "bg-red-500"
                }`}
              />
              <span className="text-sm font-medium text-gray-900">
                {healthy ? "Healthy" : "Unhealthy"}
              </span>
            </div>
          </div>
        </div>

        <div className="rounded-lg border border-gray-200 bg-white">
          <div className="px-6 py-4">
            <h2 className="text-lg font-medium text-gray-900">System Information</h2>
          </div>
          <div className="border-t border-gray-200 px-6 py-4 space-y-3">
            <div className="flex justify-between py-1.5">
              <span className="text-sm text-gray-500">Version</span>
              <span className="text-sm font-medium text-gray-900">{info.version}</span>
            </div>
            <div className="flex justify-between py-1.5">
              <span className="text-sm text-gray-500">Uptime</span>
              <span className="text-sm font-medium text-gray-900">
                {formatUptime(info.uptime_seconds)}
              </span>
            </div>
            <div className="flex justify-between py-1.5">
              <span className="text-sm text-gray-500">Storage Backend</span>
              <span className="text-sm font-medium text-gray-900">{info.storage}</span>
            </div>
            <div className="flex justify-between py-1.5">
              <span className="text-sm text-gray-500">Plugins Loaded</span>
              <span className="text-sm font-medium text-gray-900">{info.plugins_loaded}</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
