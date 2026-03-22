import { useState } from "react";
import { updateConfig } from "../../api/client";
import type { NetworkSettings } from "../../types/config";

interface Props {
  settings: NetworkSettings;
  onSaved: () => void;
  onCancel: () => void;
}

export default function NetworkSettingsForm({ settings, onSaved, onCancel }: Props) {
  const [tls, setTls] = useState({ ...settings.tls });
  const [origins, setOrigins] = useState<string[]>([...settings.cors.allowed_origins]);
  const [newOrigin, setNewOrigin] = useState("");
  const [rateLimit, setRateLimit] = useState(settings.rate_limit.requests_per_minute);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  function addOrigin() {
    const trimmed = newOrigin.trim();
    if (!trimmed) return;
    if (origins.includes(trimmed)) {
      setError("Origin already exists.");
      return;
    }
    setOrigins([...origins, trimmed]);
    setNewOrigin("");
    setError(null);
  }

  function removeOrigin(index: number) {
    setOrigins(origins.filter((_, i) => i !== index));
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);

    if (tls.enabled && !tls.cert_path.trim()) {
      setError("TLS cert path is required when TLS is enabled.");
      return;
    }
    if (tls.enabled && !tls.key_path.trim()) {
      setError("TLS key path is required when TLS is enabled.");
      return;
    }
    if (rateLimit < 0) {
      setError("Rate limit must be non-negative.");
      return;
    }

    setSaving(true);
    try {
      await updateConfig({
        network: {
          tls,
          cors: { allowed_origins: origins },
          rate_limit: { requests_per_minute: rateLimit },
        },
      });
      onSaved();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to save settings.");
    } finally {
      setSaving(false);
    }
  }

  return (
    <form onSubmit={handleSubmit} className="space-y-4">
      {error && (
        <div className="rounded-md bg-red-50 border border-red-200 px-4 py-3 text-sm text-red-700">
          {error}
        </div>
      )}

      {/* TLS */}
      <fieldset className="space-y-3">
        <legend className="text-sm font-medium text-gray-900">TLS</legend>

        <label className="flex items-center gap-2 text-sm text-gray-700">
          <input
            type="checkbox"
            checked={tls.enabled}
            onChange={(e) => setTls({ ...tls, enabled: e.target.checked })}
            className="h-4 w-4 rounded border-gray-300 text-blue-600 focus:ring-blue-500"
          />
          Enable TLS
        </label>

        {tls.enabled && (
          <>
            <div>
              <label htmlFor="net-cert" className="block text-sm font-medium text-gray-700">
                Certificate Path
              </label>
              <input
                id="net-cert"
                type="text"
                value={tls.cert_path}
                onChange={(e) => setTls({ ...tls, cert_path: e.target.value })}
                className="mt-1 block w-full rounded-md border border-gray-300 px-3 py-2 text-sm shadow-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
              />
            </div>
            <div>
              <label htmlFor="net-key" className="block text-sm font-medium text-gray-700">
                Key Path
              </label>
              <input
                id="net-key"
                type="text"
                value={tls.key_path}
                onChange={(e) => setTls({ ...tls, key_path: e.target.value })}
                className="mt-1 block w-full rounded-md border border-gray-300 px-3 py-2 text-sm shadow-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
              />
            </div>
          </>
        )}
      </fieldset>

      {/* CORS */}
      <fieldset className="space-y-3">
        <legend className="text-sm font-medium text-gray-900">CORS Allowed Origins</legend>

        {origins.length > 0 && (
          <ul className="space-y-1">
            {origins.map((origin, i) => (
              <li key={i} className="flex items-center justify-between rounded-md border border-gray-200 px-3 py-1.5 text-sm">
                <span className="text-gray-900">{origin}</span>
                <button
                  type="button"
                  onClick={() => removeOrigin(i)}
                  className="text-red-500 hover:text-red-700 text-xs font-medium"
                >
                  Remove
                </button>
              </li>
            ))}
          </ul>
        )}

        <div className="flex gap-2">
          <input
            type="text"
            value={newOrigin}
            onChange={(e) => setNewOrigin(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                addOrigin();
              }
            }}
            placeholder="https://example.com"
            className="block flex-1 rounded-md border border-gray-300 px-3 py-2 text-sm shadow-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
          />
          <button
            type="button"
            onClick={addOrigin}
            className="rounded-md border border-gray-300 bg-white px-3 py-2 text-sm font-medium text-gray-700 shadow-sm hover:bg-gray-50"
          >
            Add
          </button>
        </div>
      </fieldset>

      {/* Rate Limit */}
      <div>
        <label htmlFor="net-rate" className="block text-sm font-medium text-gray-700">
          Rate Limit (requests per minute)
        </label>
        <input
          id="net-rate"
          type="number"
          min={0}
          value={rateLimit}
          onChange={(e) => setRateLimit(Number(e.target.value))}
          className="mt-1 block w-full rounded-md border border-gray-300 px-3 py-2 text-sm shadow-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
        />
      </div>

      <div className="flex items-center gap-3 pt-2">
        <button
          type="submit"
          disabled={saving}
          className="rounded-md bg-blue-600 px-4 py-2 text-sm font-medium text-white shadow-sm hover:bg-blue-700 disabled:opacity-50"
        >
          {saving ? "Saving…" : "Save"}
        </button>
        <button
          type="button"
          onClick={onCancel}
          className="rounded-md border border-gray-300 bg-white px-4 py-2 text-sm font-medium text-gray-700 shadow-sm hover:bg-gray-50"
        >
          Cancel
        </button>
      </div>
    </form>
  );
}
