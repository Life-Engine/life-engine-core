import { useEffect, useState } from "react";
import { getPlugins, getConfig, updateConfig } from "../api/client";
import type { PluginInfo, PluginSettings } from "../types/config";

const STATUS_COLORS: Record<PluginInfo["status"], string> = {
  loaded: "bg-green-100 text-green-800",
  registered: "bg-blue-100 text-blue-800",
  failed: "bg-red-100 text-red-800",
  unloaded: "bg-gray-100 text-gray-600",
};

function PluginSettingsForm({
  settings,
  onSaved,
}: {
  settings: PluginSettings;
  onSaved: () => void;
}) {
  const [paths, setPaths] = useState<string[]>([...settings.paths]);
  const [autoEnable, setAutoEnable] = useState(settings.auto_enable);
  const [newPath, setNewPath] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  function addPath() {
    const trimmed = newPath.trim();
    if (trimmed && !paths.includes(trimmed)) {
      setPaths([...paths, trimmed]);
      setNewPath("");
    }
  }

  function removePath(index: number) {
    setPaths(paths.filter((_, i) => i !== index));
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    setSaving(true);
    try {
      await updateConfig({ plugins: { paths, auto_enable: autoEnable } });
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
        <label className="block text-sm font-medium text-gray-700">Plugin Paths</label>
        <div className="mt-1 space-y-2">
          {paths.map((p, i) => (
            <div key={i} className="flex items-center gap-2">
              <span className="flex-1 rounded-md border border-gray-200 bg-gray-50 px-3 py-2 text-sm text-gray-900">
                {p}
              </span>
              <button
                type="button"
                onClick={() => removePath(i)}
                className="rounded-md border border-gray-300 bg-white px-2 py-1.5 text-xs font-medium text-red-600 hover:bg-red-50"
              >
                Remove
              </button>
            </div>
          ))}
          <div className="flex items-center gap-2">
            <input
              type="text"
              value={newPath}
              onChange={(e) => setNewPath(e.target.value)}
              placeholder="Add plugin path…"
              className="flex-1 rounded-md border border-gray-300 px-3 py-2 text-sm shadow-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  addPath();
                }
              }}
            />
            <button
              type="button"
              onClick={addPath}
              className="rounded-md border border-gray-300 bg-white px-3 py-2 text-sm font-medium text-gray-700 shadow-sm hover:bg-gray-50"
            >
              Add
            </button>
          </div>
        </div>
      </div>

      <div className="flex items-center gap-2">
        <input
          id="auto-enable"
          type="checkbox"
          checked={autoEnable}
          onChange={(e) => setAutoEnable(e.target.checked)}
          className="h-4 w-4 rounded border-gray-300 text-blue-600 focus:ring-blue-500"
        />
        <label htmlFor="auto-enable" className="text-sm font-medium text-gray-700">
          Auto-enable new plugins
        </label>
      </div>

      <div className="flex items-center gap-3 pt-2">
        <button
          type="submit"
          disabled={saving}
          className="rounded-md bg-blue-600 px-4 py-2 text-sm font-medium text-white shadow-sm hover:bg-blue-700 disabled:opacity-50"
        >
          {saving ? "Saving…" : "Save Settings"}
        </button>
      </div>
    </form>
  );
}

export default function PluginsPage() {
  const [plugins, setPlugins] = useState<PluginInfo[]>([]);
  const [pluginSettings, setPluginSettings] = useState<PluginSettings | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  function load() {
    setLoading(true);
    setError(null);
    Promise.all([getPlugins(), getConfig()])
      .then(([pluginList, config]) => {
        setPlugins(pluginList);
        setPluginSettings(config.plugins);
      })
      .catch((err) => setError(err.message))
      .finally(() => setLoading(false));
  }

  useEffect(() => {
    load();
  }, []);

  if (loading) {
    return (
      <div className="flex items-center gap-2 text-gray-500">
        <svg className="h-5 w-5 animate-spin" viewBox="0 0 24 24" fill="none">
          <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
          <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v4a4 4 0 00-4 4H4z" />
        </svg>
        Loading plugins…
      </div>
    );
  }

  if (error) {
    return (
      <div className="rounded-lg border border-red-200 bg-red-50 p-4 text-sm text-red-700">
        Failed to load plugins: {error}
      </div>
    );
  }

  return (
    <div>
      <div className="mb-6">
        <h1 className="text-2xl font-semibold text-gray-900">Plugins</h1>
        <p className="mt-1 text-sm text-gray-600">
          Loaded plugins and plugin configuration settings.
        </p>
      </div>

      <div className="space-y-6">
        <div className="rounded-lg border border-gray-200 bg-white">
          <div className="px-6 py-4">
            <h2 className="text-lg font-medium text-gray-900">Loaded Plugins</h2>
          </div>
          <div className="border-t border-gray-200">
            {plugins.length === 0 ? (
              <div className="px-6 py-8 text-center text-sm text-gray-500">
                No plugins loaded.
              </div>
            ) : (
              <ul className="divide-y divide-gray-200">
                {plugins.map((plugin) => (
                  <li key={plugin.id} className="flex items-center justify-between px-6 py-4">
                    <div>
                      <p className="text-sm font-medium text-gray-900">{plugin.name}</p>
                      <p className="text-xs text-gray-500">
                        {plugin.id} — v{plugin.version}
                      </p>
                    </div>
                    <span
                      className={`inline-flex rounded-full px-2.5 py-0.5 text-xs font-medium ${STATUS_COLORS[plugin.status]}`}
                    >
                      {plugin.status}
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </div>

        {pluginSettings && (
          <div className="rounded-lg border border-gray-200 bg-white">
            <div className="px-6 py-4">
              <h2 className="text-lg font-medium text-gray-900">Plugin Settings</h2>
            </div>
            <div className="border-t border-gray-200 px-6 py-4">
              <PluginSettingsForm settings={pluginSettings} onSaved={load} />
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
