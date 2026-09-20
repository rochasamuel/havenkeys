import { useCallback, useEffect, useState } from "react";
import { api } from "../lib/api";
import type { DeviceStatus } from "../lib/types";
import { useToast } from "../components/Toast";
import { EmergencyKit } from "../components/EmergencyKit";

export function SyncSection() {
  const toast = useToast();
  const [device, setDevice] = useState<DeviceStatus | null>(null);
  const [showKit, setShowKit] = useState(false);

  const refresh = useCallback(() => {
    api.deviceStatus().then(setDevice, () => toast("Could not read sync settings.", "error"));
  }, [toast]);
  useEffect(refresh, [refresh]);

  if (!device) return null;

  return (
    <div className="settings-block">
      <h3>Secret Key</h3>

      {!showKit && (
        <div>
          <button className="btn" type="button" onClick={() => setShowKit(true)}>
            Show Emergency Kit
          </button>
        </div>
      )}
      {showKit && <EmergencyKit onDone={() => setShowKit(false)} />}
    </div>
  );
}
