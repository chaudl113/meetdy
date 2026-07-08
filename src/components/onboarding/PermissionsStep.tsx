import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { type as getOsType } from "@tauri-apps/plugin-os";
import {
  checkMicrophonePermission,
  requestMicrophonePermission,
  checkScreenRecordingPermission,
  requestScreenRecordingPermission,
} from "tauri-plugin-macos-permissions-api";

interface PermissionsStepProps {
  onContinue: () => void;
}

const PermissionRow: React.FC<{
  label: string;
  description: string;
  granted: boolean;
  onGrant: () => void;
  grantLabel: string;
  grantedLabel: string;
  notGrantedLabel: string;
}> = ({
  label,
  description,
  granted,
  onGrant,
  grantLabel,
  grantedLabel,
  notGrantedLabel,
}) => (
  <div className="flex items-center justify-between gap-4 rounded-lg border border-mid-gray/20 bg-mid-gray/[0.04] px-4 py-3">
    <div className="min-w-0 flex-1">
      <div className="flex items-center gap-2">
        <span className="text-sm font-medium text-text">{label}</span>
        {granted ? (
          <span className="rounded-full bg-green-500/15 px-2 py-0.5 text-xs font-medium text-green-400">
            {grantedLabel}
          </span>
        ) : (
          <span className="rounded-full bg-mid-gray/20 px-2 py-0.5 text-xs font-medium text-text/50">
            {notGrantedLabel}
          </span>
        )}
      </div>
      <p className="mt-0.5 text-xs text-text/50">{description}</p>
    </div>
    {!granted && (
      <button
        type="button"
        onClick={onGrant}
        className="shrink-0 rounded-md bg-logo-primary/10 border border-logo-primary/30 px-3 py-1.5 text-xs font-medium text-logo-primary hover:bg-logo-primary/20 transition-colors"
      >
        {grantLabel}
      </button>
    )}
  </div>
);

export const PermissionsStep: React.FC<PermissionsStepProps> = ({
  onContinue,
}) => {
  const { t } = useTranslation();
  const [isMacos, setIsMacos] = useState(false);
  const [micGranted, setMicGranted] = useState(false);
  const [screenGranted, setScreenGranted] = useState(false);
  const [checking, setChecking] = useState(true);

  useEffect(() => {
    const init = async () => {
      const osType = await getOsType();
      const macos = osType === "macos";
      setIsMacos(macos);

      if (!macos) {
        // Non-macOS: auto-grant everything and skip
        setMicGranted(true);
        setScreenGranted(true);
        setChecking(false);
        return;
      }

      const [mic, screen] = await Promise.all([
        checkMicrophonePermission().catch(() => false),
        checkScreenRecordingPermission().catch(() => false),
      ]);
      setMicGranted(mic);
      setScreenGranted(screen);
      setChecking(false);
    };
    init();
  }, []);

  // Auto-continue on non-macOS
  useEffect(() => {
    if (!checking && !isMacos) {
      onContinue();
    }
  }, [checking, isMacos, onContinue]);

  const handleGrantMic = async () => {
    try {
      await requestMicrophonePermission();
    } catch {
      // ignore — user may need to go to system settings
    }
    // Re-check
    const granted = await checkMicrophonePermission().catch(() => false);
    setMicGranted(granted);
  };

  const handleGrantScreen = async () => {
    try {
      await requestScreenRecordingPermission();
    } catch {
      // ignore
    }
    const granted = await checkScreenRecordingPermission().catch(() => false);
    setScreenGranted(granted);
  };

  const bothGranted = micGranted && screenGranted;

  if (checking || !isMacos) {
    return null;
  }

  return (
    <div className="flex flex-col gap-6">
      <div className="text-center">
        <h2 className="text-lg font-semibold text-text">
          {t("onboarding.permissions.title", "Permissions")}
        </h2>
        <p className="mt-1 text-sm text-text/60">
          {t(
            "onboarding.permissions.subtitle",
            "Meetdy needs the following permissions to work properly",
          )}
        </p>
      </div>

      <div className="flex flex-col gap-3">
        <PermissionRow
          label={t("onboarding.permissions.mic", "Microphone")}
          description={t(
            "onboarding.permissions.micDescription",
            "Required to capture your voice during meetings",
          )}
          granted={micGranted}
          onGrant={handleGrantMic}
          grantLabel={t("onboarding.permissions.grantAccess", "Grant Access")}
          grantedLabel={t("onboarding.permissions.granted", "Granted")}
          notGrantedLabel={t(
            "onboarding.permissions.notGranted",
            "Not Granted",
          )}
        />
        <PermissionRow
          label={t(
            "onboarding.permissions.screenRecording",
            "Screen Recording",
          )}
          description={t(
            "onboarding.permissions.screenRecordingDescription",
            "Required to capture system audio (e.g. from Zoom, YouTube)",
          )}
          granted={screenGranted}
          onGrant={handleGrantScreen}
          grantLabel={t("onboarding.permissions.grantAccess", "Grant Access")}
          grantedLabel={t("onboarding.permissions.granted", "Granted")}
          notGrantedLabel={t(
            "onboarding.permissions.notGranted",
            "Not Granted",
          )}
        />
      </div>

      <div className="flex flex-col items-center gap-2">
        <button
          type="button"
          onClick={onContinue}
          disabled={!bothGranted}
          className="w-full rounded-lg bg-logo-primary px-4 py-2.5 text-sm font-semibold text-white transition-opacity disabled:opacity-40 hover:opacity-90"
        >
          {t("onboarding.permissions.continue", "Continue")}
        </button>
        <button
          type="button"
          onClick={onContinue}
          className="text-xs text-text/40 hover:text-text/60 transition-colors"
        >
          {t("onboarding.permissions.skip", "Skip for now")}
        </button>
      </div>
    </div>
  );
};

export default PermissionsStep;
