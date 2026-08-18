import { useEffect, useState } from "react";
import { RefreshCwIcon } from "lucide-react";
import { getVersion } from "@tauri-apps/api/app";
import { isTauri } from "@/lib/tauri";
import { useUpdateChecker } from "../hooks/useUpdateChecker";

export function AboutTab() {
  const [appVersion, setAppVersion] = useState("0.1.7");
  const { status, availableVersion, progress, error, checkForUpdates, installUpdate } = useUpdateChecker();

  useEffect(() => {
    if (isTauri) void getVersion().then(setAppVersion).catch(() => undefined);
  }, []);

  return (
    <div className="v3-settings__tab-panel">
      <div className="v3-settings__heading">
        <h2>About Nitro</h2>
        <p>A focused space for one-to-one chat, voice, and shared moments.</p>
      </div>

      <div className="v3-settings__panel">
        <div className="v3-settings__panel-section">
          <div className="v3-settings__about-head">
            <div>
              <p className="v3-settings__row-title">Nitro {appVersion}</p>
              <p className="v3-settings__row-desc">{isTauri ? "Desktop release channel · signed updates" : "Web app"}</p>
            </div>
            <div className="v3-settings__about-actions">
              <button
                type="button"
                className="v3-settings__ghost-button"
                disabled={status === "checking" || status === "downloading"}
                onClick={() => void checkForUpdates()}
              >
                <RefreshCwIcon aria-hidden="true" className={status === "checking" ? "v3-settings__spin" : undefined} />
                Check now
              </button>
              {status === "available" && (
                <button type="button" className="v3-settings__save" onClick={() => void installUpdate()}>
                  Install {availableVersion}
                </button>
              )}
            </div>
          </div>
          {status === "downloading" ? (
            <div className="v3-settings__update-progress">
              <div className="v3-settings__storage-bar">
                <div className="v3-settings__storage-bar-fill" style={{ width: (progress ?? 0) + "%" }} />
              </div>
              <p className="v3-settings__row-desc" style={{ marginTop: 8 }}>
                Downloading signed update{progress === null ? "…" : ` · ${progress}%`}
              </p>
            </div>
          ) : (
            <p className="v3-settings__row-desc" style={{ marginTop: 14 }}>
              {status === "available"
                ? "Update " + availableVersion + " is ready to install in place."
                : status === "current"
                  ? "You are up to date."
                  : status === "error"
                    ? "Could not check for updates: " + (error ?? "Unknown updater error.")
                    : "Nitro checks for updates on startup and lets you know - nothing installs without your say-so. You can also check manually here, or with /update in a chat."}
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
