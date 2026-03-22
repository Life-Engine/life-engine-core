import { useState } from "react";
import { updateConfig } from "../../api/client";
import type { CoreSettings } from "../../types/config";

const LOG_LEVELS = ["trace", "debug", "info", "warn", "error"];
const LOG_FORMATS = ["pretty", "json"];

interface Props {
  settings: CoreSettings;
  onSaved: () => void;
  onCancel: () => void;
}

export default function CoreSettingsForm({ settings, onSaved, onCancel }: Props) {
  const [form, setForm] = useState<CoreSettings>({ ...settings });
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  function set<K extends keyof CoreSettings>(key: K, value: CoreSettings[K]) {
    setForm((prev) => ({ ...prev, [key]: value }));
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);

    if (!form.host.trim()) {
      setError("Host is required.");
      return;
    }
    if (form.port < 1 || form.port > 65535) {
      setError("Port must be between 1 and 65535.");
      return;
    }
    if (!form.data_dir.trim()) {
      setError("Data directory is required.");
      return;
    }

    setSaving(true);
    try {
      await updateConfig({ core: form });
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

      <div>
        <label htmlFor="core-host" className="block text-sm font-medium text-gray-700">
          Host
        </label>
        <input
          id="core-host"
          type="text"
          value={form.host}
          onChange={(e) => set("host", e.target.value)}
          className="mt-1 block w-full rounded-md border border-gray-300 px-3 py-2 text-sm shadow-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
        />
      </div>

      <div>
        <label htmlFor="core-port" className="block text-sm font-medium text-gray-700">
          Port
        </label>
        <input
          id="core-port"
          type="number"
          min={1}
          max={65535}
          value={form.port}
          onChange={(e) => set("port", Number(e.target.value))}
          className="mt-1 block w-full rounded-md border border-gray-300 px-3 py-2 text-sm shadow-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
        />
      </div>

      <div>
        <label htmlFor="core-log-level" className="block text-sm font-medium text-gray-700">
          Log Level
        </label>
        <select
          id="core-log-level"
          value={form.log_level}
          onChange={(e) => set("log_level", e.target.value)}
          className="mt-1 block w-full rounded-md border border-gray-300 px-3 py-2 text-sm shadow-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
        >
          {LOG_LEVELS.map((level) => (
            <option key={level} value={level}>
              {level}
            </option>
          ))}
        </select>
      </div>

      <div>
        <label htmlFor="core-log-format" className="block text-sm font-medium text-gray-700">
          Log Format
        </label>
        <select
          id="core-log-format"
          value={form.log_format}
          onChange={(e) => set("log_format", e.target.value)}
          className="mt-1 block w-full rounded-md border border-gray-300 px-3 py-2 text-sm shadow-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
        >
          {LOG_FORMATS.map((fmt) => (
            <option key={fmt} value={fmt}>
              {fmt}
            </option>
          ))}
        </select>
      </div>

      <div>
        <label htmlFor="core-data-dir" className="block text-sm font-medium text-gray-700">
          Data Directory
        </label>
        <input
          id="core-data-dir"
          type="text"
          value={form.data_dir}
          onChange={(e) => set("data_dir", e.target.value)}
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
