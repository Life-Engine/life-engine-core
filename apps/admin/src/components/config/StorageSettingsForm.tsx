import { useState } from "react";
import { updateConfig } from "../../api/client";
import type { StorageSettings, Argon2Settings, PostgresSettings } from "../../types/config";

const BACKENDS = ["sqlite", "postgres"] as const;
const SSL_MODES = ["Disable", "Prefer", "Require"] as const;

const DEFAULT_POSTGRES: PostgresSettings = {
  host: "localhost",
  port: 5432,
  dbname: "life_engine",
  user: "life_engine",
  password: "",
  pool_size: 10,
  ssl_mode: "Prefer",
};

interface Props {
  settings: StorageSettings;
  onSaved: () => void;
  onCancel: () => void;
}

export default function StorageSettingsForm({ settings, onSaved, onCancel }: Props) {
  const [backend, setBackend] = useState(settings.backend);
  const [encryption, setEncryption] = useState(settings.encryption);
  const [argon2, setArgon2] = useState<Argon2Settings>({ ...settings.argon2 });
  const [postgres, setPostgres] = useState<PostgresSettings>(settings.postgres ?? { ...DEFAULT_POSTGRES });
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);

    if (argon2.memory_mb < 1) {
      setError("Argon2 memory must be at least 1 MB.");
      return;
    }
    if (argon2.iterations < 1) {
      setError("Argon2 iterations must be at least 1.");
      return;
    }
    if (argon2.parallelism < 1) {
      setError("Argon2 parallelism must be at least 1.");
      return;
    }

    if (backend === "postgres") {
      if (!postgres.host.trim()) {
        setError("PostgreSQL host is required.");
        return;
      }
      if (postgres.port < 1 || postgres.port > 65535) {
        setError("PostgreSQL port must be between 1 and 65535.");
        return;
      }
      if (!postgres.dbname.trim()) {
        setError("PostgreSQL database name is required.");
        return;
      }
      if (!postgres.user.trim()) {
        setError("PostgreSQL user is required.");
        return;
      }
      if (postgres.pool_size < 1) {
        setError("Pool size must be at least 1.");
        return;
      }
    }

    setSaving(true);
    try {
      await updateConfig({
        storage: {
          backend,
          encryption,
          argon2,
          postgres: backend === "postgres" ? postgres : null,
        },
      });
      onSaved();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to save settings.");
    } finally {
      setSaving(false);
    }
  }

  const inputClass =
    "mt-1 block w-full rounded-md border border-gray-300 px-3 py-2 text-sm shadow-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500";

  return (
    <form onSubmit={handleSubmit} className="space-y-4">
      {error && (
        <div className="rounded-md bg-red-50 border border-red-200 px-4 py-3 text-sm text-red-700">
          {error}
        </div>
      )}

      <fieldset className="space-y-3">
        <legend className="text-sm font-medium text-gray-900">Storage Backend</legend>
        <div className="flex gap-4">
          {BACKENDS.map((b) => (
            <label key={b} className="flex items-center gap-2 text-sm text-gray-700">
              <input
                type="radio"
                name="storage-backend"
                value={b}
                checked={backend === b}
                onChange={() => setBackend(b)}
                className="h-4 w-4 border-gray-300 text-blue-600 focus:ring-blue-500"
              />
              {b}
            </label>
          ))}
        </div>
      </fieldset>

      <label className="flex items-center gap-2 text-sm text-gray-700">
        <input
          type="checkbox"
          checked={encryption}
          onChange={(e) => setEncryption(e.target.checked)}
          className="h-4 w-4 rounded border-gray-300 text-blue-600 focus:ring-blue-500"
        />
        Enable encryption
      </label>

      <fieldset className="space-y-3">
        <legend className="text-sm font-medium text-gray-900">Argon2 Key Derivation</legend>

        <div>
          <label htmlFor="stor-argon-mem" className="block text-sm font-medium text-gray-700">
            Memory (MB)
          </label>
          <input
            id="stor-argon-mem"
            type="number"
            min={1}
            value={argon2.memory_mb}
            onChange={(e) => setArgon2({ ...argon2, memory_mb: Number(e.target.value) })}
            className={inputClass}
          />
        </div>

        <div>
          <label htmlFor="stor-argon-iter" className="block text-sm font-medium text-gray-700">
            Iterations
          </label>
          <input
            id="stor-argon-iter"
            type="number"
            min={1}
            value={argon2.iterations}
            onChange={(e) => setArgon2({ ...argon2, iterations: Number(e.target.value) })}
            className={inputClass}
          />
        </div>

        <div>
          <label htmlFor="stor-argon-par" className="block text-sm font-medium text-gray-700">
            Parallelism
          </label>
          <input
            id="stor-argon-par"
            type="number"
            min={1}
            value={argon2.parallelism}
            onChange={(e) => setArgon2({ ...argon2, parallelism: Number(e.target.value) })}
            className={inputClass}
          />
        </div>
      </fieldset>

      {backend === "postgres" && (
        <fieldset className="space-y-3">
          <legend className="text-sm font-medium text-gray-900">PostgreSQL Connection</legend>

          <div>
            <label htmlFor="stor-pg-host" className="block text-sm font-medium text-gray-700">
              Host
            </label>
            <input
              id="stor-pg-host"
              type="text"
              value={postgres.host}
              onChange={(e) => setPostgres({ ...postgres, host: e.target.value })}
              className={inputClass}
            />
          </div>

          <div>
            <label htmlFor="stor-pg-port" className="block text-sm font-medium text-gray-700">
              Port
            </label>
            <input
              id="stor-pg-port"
              type="number"
              min={1}
              max={65535}
              value={postgres.port}
              onChange={(e) => setPostgres({ ...postgres, port: Number(e.target.value) })}
              className={inputClass}
            />
          </div>

          <div>
            <label htmlFor="stor-pg-db" className="block text-sm font-medium text-gray-700">
              Database
            </label>
            <input
              id="stor-pg-db"
              type="text"
              value={postgres.dbname}
              onChange={(e) => setPostgres({ ...postgres, dbname: e.target.value })}
              className={inputClass}
            />
          </div>

          <div>
            <label htmlFor="stor-pg-user" className="block text-sm font-medium text-gray-700">
              User
            </label>
            <input
              id="stor-pg-user"
              type="text"
              value={postgres.user}
              onChange={(e) => setPostgres({ ...postgres, user: e.target.value })}
              className={inputClass}
            />
          </div>

          <div>
            <label htmlFor="stor-pg-pass" className="block text-sm font-medium text-gray-700">
              Password
            </label>
            <input
              id="stor-pg-pass"
              type="password"
              value={postgres.password}
              onChange={(e) => setPostgres({ ...postgres, password: e.target.value })}
              placeholder="[REDACTED]"
              className={inputClass}
            />
          </div>

          <div>
            <label htmlFor="stor-pg-pool" className="block text-sm font-medium text-gray-700">
              Pool Size
            </label>
            <input
              id="stor-pg-pool"
              type="number"
              min={1}
              value={postgres.pool_size}
              onChange={(e) => setPostgres({ ...postgres, pool_size: Number(e.target.value) })}
              className={inputClass}
            />
          </div>

          <div>
            <label htmlFor="stor-pg-ssl" className="block text-sm font-medium text-gray-700">
              SSL Mode
            </label>
            <select
              id="stor-pg-ssl"
              value={postgres.ssl_mode}
              onChange={(e) => setPostgres({ ...postgres, ssl_mode: e.target.value as PostgresSettings["ssl_mode"] })}
              className={inputClass}
            >
              {SSL_MODES.map((mode) => (
                <option key={mode} value={mode}>
                  {mode}
                </option>
              ))}
            </select>
          </div>
        </fieldset>
      )}

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
