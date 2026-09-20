import { useCallback, useEffect, useState, type FormEvent } from "react";
import { api, ApiError } from "../lib/api";
import type { DeviceStatus } from "../lib/types";
import { useToast } from "../components/Toast";
import { EmergencyKit } from "../components/EmergencyKit";

/** Add a Secret Key to a vault created before it existed. */
function AddSecretKey({ onDone }: { onDone: () => void }) {
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (busy || !password) return;
    setBusy(true);
    setError(null);
    try {
      await api.setupSecretKey(password);
      setPassword("");
      onDone();
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not add a Secret Key.");
      setBusy(false);
    }
  }

  return (
    <form onSubmit={submit} className="settings-inline">
      <p className="muted">
        This vault is protected by your master password only. Adding a Secret Key means a copy of the vault (a
        backup, or a copy synced elsewhere) cannot be opened with the password alone.
      </p>
      <label className="control">
        <span>Master password</span>
        <input
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          autoComplete="off"
          spellCheck={false}
          disabled={busy}
        />
      </label>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      <div>
        <button className="btn btn-primary" type="submit" disabled={busy || !password}>
          {busy ? "Adding…" : "Add a Secret Key"}
        </button>
      </div>
    </form>
  );
}

export function SyncSection() {
  const toast = useToast();
  const [device, setDevice] = useState<DeviceStatus | null>(null);
  const [kit, setKit] = useState<"hidden" | "shown" | "first">("hidden");

  const refresh = useCallback(() => {
    api.deviceStatus().then(setDevice, () => toast("Could not read sync settings.", "error"));
  }, [toast]);
  useEffect(refresh, [refresh]);

  if (!device) return null;
  const hasKey = device.keyScheme === "password_and_secret_key" || device.keyScheme === "account_bound";

  return (
    <div className="settings-block">
      <h3>Secret Key</h3>

      {!hasKey && kit === "hidden" && (
        <AddSecretKey
          onDone={() => {
            setKit("first");
            refresh();
          }}
        />
      )}

      {hasKey && kit === "hidden" && (
        <div>
          <button className="btn" type="button" onClick={() => setKit("shown")}>
            Show Emergency Kit
          </button>
        </div>
      )}
      {kit !== "hidden" && (
        <EmergencyKit onDone={() => setKit("hidden")} />
      )}
    </div>
  );
}
