import { useEffect, useState } from "react";
import { getConfig } from "../api/client";
import type { CoreConfig } from "../types/config";
import CoreSettingsForm from "../components/config/CoreSettingsForm";
import NetworkSettingsForm from "../components/config/NetworkSettingsForm";

type EditingSection = "core" | "network" | null;

function Section({
  title,
  defaultOpen = false,
  onEdit,
  children,
}: {
  title: string;
  defaultOpen?: boolean;
  onEdit?: () => void;
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(defaultOpen);

  return (
    <div className="rounded-lg border border-gray-200 bg-white">
      <div className="flex items-center justify-between px-6 py-4">
        <button
          type="button"
          className="flex flex-1 items-center justify-between text-left"
          onClick={() => setOpen(!open)}
        >
          <h2 className="text-lg font-medium text-gray-900">{title}</h2>
          <svg
            className={`h-5 w-5 text-gray-500 transition-transform ${open ? "rotate-180" : ""}`}
            fill="none"
            viewBox="0 0 24 24"
            strokeWidth={1.5}
            stroke="currentColor"
          >
            <path strokeLinecap="round" strokeLinejoin="round" d="M19.5 8.25l-7.5 7.5-7.5-7.5" />
          </svg>
        </button>
        {onEdit && (
          <button
            type="button"
            onClick={onEdit}
            className="ml-4 rounded-md border border-gray-300 bg-white px-3 py-1.5 text-xs font-medium text-gray-700 shadow-sm hover:bg-gray-50"
          >
            Edit
          </button>
        )}
      </div>
      {open && <div className="border-t border-gray-200 px-6 py-4">{children}</div>}
    </div>
  );
}

function Field({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between py-1.5">
      <span className="text-sm text-gray-500">{label}</span>
      <span className="text-sm font-medium text-gray-900">{value}</span>
    </div>
  );
}

function CoreSection({ config }: { config: CoreConfig }) {
  const { core } = config;
  return (
    <div className="space-y-1">
      <Field label="Host" value={core.host} />
      <Field label="Port" value={String(core.port)} />
      <Field label="Log Level" value={core.log_level} />
      <Field label="Log Format" value={core.log_format} />
      <Field label="Data Directory" value={core.data_dir} />
    </div>
  );
}

function AuthSection({ config }: { config: CoreConfig }) {
  const { auth } = config;
  return (
    <div className="space-y-1">
      <Field label="Provider" value={auth.provider} />
      {auth.oidc && (
        <>
          <Field label="OIDC Issuer URL" value={auth.oidc.issuer_url} />
          <Field label="OIDC Client ID" value={auth.oidc.client_id} />
          <Field label="OIDC Client Secret" value={auth.oidc.client_secret ?? "—"} />
          {auth.oidc.jwks_uri && <Field label="JWKS URI" value={auth.oidc.jwks_uri} />}
          {auth.oidc.audience && <Field label="Audience" value={auth.oidc.audience} />}
        </>
      )}
      {auth.webauthn && (
        <>
          <Field label="WebAuthn RP Name" value={auth.webauthn.rp_name} />
          <Field label="WebAuthn RP ID" value={auth.webauthn.rp_id} />
          <Field label="WebAuthn RP Origin" value={auth.webauthn.rp_origin} />
          <Field label="Challenge TTL (seconds)" value={String(auth.webauthn.challenge_ttl_secs)} />
        </>
      )}
    </div>
  );
}

function StorageSection({ config }: { config: CoreConfig }) {
  const { storage } = config;
  return (
    <div className="space-y-1">
      <Field label="Backend" value={storage.backend} />
      <Field label="Encryption" value={storage.encryption ? "Enabled" : "Disabled"} />
      <Field label="Argon2 Memory (MB)" value={String(storage.argon2.memory_mb)} />
      <Field label="Argon2 Iterations" value={String(storage.argon2.iterations)} />
      <Field label="Argon2 Parallelism" value={String(storage.argon2.parallelism)} />
      {storage.postgres && (
        <>
          <Field label="PostgreSQL Host" value={storage.postgres.host} />
          <Field label="PostgreSQL Port" value={String(storage.postgres.port)} />
          <Field label="PostgreSQL Database" value={storage.postgres.dbname} />
          <Field label="PostgreSQL User" value={storage.postgres.user} />
          <Field label="PostgreSQL Password" value={storage.postgres.password} />
          <Field label="PostgreSQL Pool Size" value={String(storage.postgres.pool_size)} />
          <Field label="PostgreSQL SSL Mode" value={storage.postgres.ssl_mode} />
        </>
      )}
    </div>
  );
}

function PluginsSection({ config }: { config: CoreConfig }) {
  const { plugins } = config;
  return (
    <div className="space-y-1">
      <Field label="Auto Enable" value={plugins.auto_enable ? "Yes" : "No"} />
      <div className="flex justify-between py-1.5">
        <span className="text-sm text-gray-500">Paths</span>
        <span className="text-sm font-medium text-gray-900">
          {plugins.paths.length > 0 ? plugins.paths.join(", ") : "—"}
        </span>
      </div>
    </div>
  );
}

function NetworkSection({ config }: { config: CoreConfig }) {
  const { network } = config;
  return (
    <div className="space-y-1">
      <Field label="TLS Enabled" value={network.tls.enabled ? "Yes" : "No"} />
      {network.tls.enabled && (
        <>
          <Field label="TLS Cert Path" value={network.tls.cert_path} />
          <Field label="TLS Key Path" value={network.tls.key_path} />
        </>
      )}
      <Field label="Rate Limit (req/min)" value={String(network.rate_limit.requests_per_minute)} />
      <div className="flex justify-between py-1.5">
        <span className="text-sm text-gray-500">CORS Allowed Origins</span>
        <span className="text-sm font-medium text-gray-900">
          {network.cors.allowed_origins.length > 0
            ? network.cors.allowed_origins.join(", ")
            : "—"}
        </span>
      </div>
    </div>
  );
}

export default function ConfigPage() {
  const [config, setConfig] = useState<CoreConfig | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [editing, setEditing] = useState<EditingSection>(null);

  function reload() {
    setLoading(true);
    setError(null);
    getConfig()
      .then(setConfig)
      .catch((err) => setError(err.message))
      .finally(() => setLoading(false));
  }

  useEffect(() => {
    reload();
  }, []);

  if (loading) {
    return (
      <div className="flex items-center gap-2 text-gray-500">
        <svg className="h-5 w-5 animate-spin" viewBox="0 0 24 24" fill="none">
          <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
          <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v4a4 4 0 00-4 4H4z" />
        </svg>
        Loading configuration…
      </div>
    );
  }

  if (error) {
    return (
      <div className="rounded-lg border border-red-200 bg-red-50 p-4 text-sm text-red-700">
        Failed to load configuration: {error}
      </div>
    );
  }

  if (!config) return null;

  return (
    <div>
      <div className="mb-6">
        <h1 className="text-2xl font-semibold text-gray-900">Configuration</h1>
        <p className="mt-1 text-sm text-gray-600">
          View and edit runtime configuration. Expand a section to see current values.
        </p>
      </div>

      <div className="space-y-4">
        <Section title="Core" defaultOpen onEdit={editing ? undefined : () => setEditing("core")}>
          {editing === "core" ? (
            <CoreSettingsForm
              settings={config.core}
              onSaved={() => { setEditing(null); reload(); }}
              onCancel={() => setEditing(null)}
            />
          ) : (
            <CoreSection config={config} />
          )}
        </Section>

        <Section title="Authentication">
          <AuthSection config={config} />
        </Section>

        <Section title="Storage">
          <StorageSection config={config} />
        </Section>

        <Section title="Plugins">
          <PluginsSection config={config} />
        </Section>

        <Section title="Network" onEdit={editing ? undefined : () => setEditing("network")}>
          {editing === "network" ? (
            <NetworkSettingsForm
              settings={config.network}
              onSaved={() => { setEditing(null); reload(); }}
              onCancel={() => setEditing(null)}
            />
          ) : (
            <NetworkSection config={config} />
          )}
        </Section>
      </div>
    </div>
  );
}
