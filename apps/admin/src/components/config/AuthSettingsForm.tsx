import { useState } from "react";
import { updateConfig } from "../../api/client";
import type { AuthSettings, OidcSettings, WebAuthnSettings } from "../../types/config";

const PROVIDERS = ["local-token", "oidc", "webauthn"] as const;

const DEFAULT_OIDC: OidcSettings = {
  issuer_url: "",
  client_id: "",
  client_secret: null,
  jwks_uri: null,
  audience: null,
};

const DEFAULT_WEBAUTHN: WebAuthnSettings = {
  rp_name: "",
  rp_id: "",
  rp_origin: "",
  challenge_ttl_secs: 300,
};

interface Props {
  settings: AuthSettings;
  onSaved: () => void;
  onCancel: () => void;
}

export default function AuthSettingsForm({ settings, onSaved, onCancel }: Props) {
  const [provider, setProvider] = useState(settings.provider);
  const [oidc, setOidc] = useState<OidcSettings>(settings.oidc ?? { ...DEFAULT_OIDC });
  const [webauthn, setWebauthn] = useState<WebAuthnSettings>(settings.webauthn ?? { ...DEFAULT_WEBAUTHN });
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);

    if (provider === "oidc") {
      if (!oidc.issuer_url.trim()) {
        setError("OIDC Issuer URL is required.");
        return;
      }
      if (!oidc.client_id.trim()) {
        setError("OIDC Client ID is required.");
        return;
      }
    }

    if (provider === "webauthn") {
      if (!webauthn.rp_name.trim()) {
        setError("WebAuthn RP Name is required.");
        return;
      }
      if (!webauthn.rp_id.trim()) {
        setError("WebAuthn RP ID is required.");
        return;
      }
      if (!webauthn.rp_origin.trim()) {
        setError("WebAuthn RP Origin is required.");
        return;
      }
    }

    setSaving(true);
    try {
      await updateConfig({
        auth: {
          provider,
          oidc: provider === "oidc" ? oidc : null,
          webauthn: provider === "webauthn" ? webauthn : null,
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
        <legend className="text-sm font-medium text-gray-900">Auth Provider</legend>
        <div className="flex gap-4">
          {PROVIDERS.map((p) => (
            <label key={p} className="flex items-center gap-2 text-sm text-gray-700">
              <input
                type="radio"
                name="auth-provider"
                value={p}
                checked={provider === p}
                onChange={() => setProvider(p)}
                className="h-4 w-4 border-gray-300 text-blue-600 focus:ring-blue-500"
              />
              {p}
            </label>
          ))}
        </div>
      </fieldset>

      {provider === "oidc" && (
        <fieldset className="space-y-3">
          <legend className="text-sm font-medium text-gray-900">OIDC Settings</legend>

          <div>
            <label htmlFor="auth-oidc-issuer" className="block text-sm font-medium text-gray-700">
              Issuer URL
            </label>
            <input
              id="auth-oidc-issuer"
              type="text"
              value={oidc.issuer_url}
              onChange={(e) => setOidc({ ...oidc, issuer_url: e.target.value })}
              className={inputClass}
            />
          </div>

          <div>
            <label htmlFor="auth-oidc-client-id" className="block text-sm font-medium text-gray-700">
              Client ID
            </label>
            <input
              id="auth-oidc-client-id"
              type="text"
              value={oidc.client_id}
              onChange={(e) => setOidc({ ...oidc, client_id: e.target.value })}
              className={inputClass}
            />
          </div>

          <div>
            <label htmlFor="auth-oidc-secret" className="block text-sm font-medium text-gray-700">
              Client Secret
            </label>
            <input
              id="auth-oidc-secret"
              type="password"
              value={oidc.client_secret ?? ""}
              onChange={(e) => setOidc({ ...oidc, client_secret: e.target.value || null })}
              placeholder="[REDACTED]"
              className={inputClass}
            />
          </div>

          <div>
            <label htmlFor="auth-oidc-jwks" className="block text-sm font-medium text-gray-700">
              JWKS URI (optional)
            </label>
            <input
              id="auth-oidc-jwks"
              type="text"
              value={oidc.jwks_uri ?? ""}
              onChange={(e) => setOidc({ ...oidc, jwks_uri: e.target.value || null })}
              className={inputClass}
            />
          </div>

          <div>
            <label htmlFor="auth-oidc-audience" className="block text-sm font-medium text-gray-700">
              Audience (optional)
            </label>
            <input
              id="auth-oidc-audience"
              type="text"
              value={oidc.audience ?? ""}
              onChange={(e) => setOidc({ ...oidc, audience: e.target.value || null })}
              className={inputClass}
            />
          </div>
        </fieldset>
      )}

      {provider === "webauthn" && (
        <fieldset className="space-y-3">
          <legend className="text-sm font-medium text-gray-900">WebAuthn Settings</legend>

          <div>
            <label htmlFor="auth-wa-rp-name" className="block text-sm font-medium text-gray-700">
              RP Name
            </label>
            <input
              id="auth-wa-rp-name"
              type="text"
              value={webauthn.rp_name}
              onChange={(e) => setWebauthn({ ...webauthn, rp_name: e.target.value })}
              className={inputClass}
            />
          </div>

          <div>
            <label htmlFor="auth-wa-rp-id" className="block text-sm font-medium text-gray-700">
              RP ID
            </label>
            <input
              id="auth-wa-rp-id"
              type="text"
              value={webauthn.rp_id}
              onChange={(e) => setWebauthn({ ...webauthn, rp_id: e.target.value })}
              className={inputClass}
            />
          </div>

          <div>
            <label htmlFor="auth-wa-rp-origin" className="block text-sm font-medium text-gray-700">
              RP Origin
            </label>
            <input
              id="auth-wa-rp-origin"
              type="text"
              value={webauthn.rp_origin}
              onChange={(e) => setWebauthn({ ...webauthn, rp_origin: e.target.value })}
              className={inputClass}
            />
          </div>

          <div>
            <label htmlFor="auth-wa-ttl" className="block text-sm font-medium text-gray-700">
              Challenge TTL (seconds)
            </label>
            <input
              id="auth-wa-ttl"
              type="number"
              min={1}
              value={webauthn.challenge_ttl_secs}
              onChange={(e) => setWebauthn({ ...webauthn, challenge_ttl_secs: Number(e.target.value) })}
              className={inputClass}
            />
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
