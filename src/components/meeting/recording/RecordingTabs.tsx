import React from "react";
import { useTranslation } from "react-i18next";
import { FileText, Sparkles, StickyNote } from "lucide-react";

export type RecordingTab = "transcript" | "summary" | "notes";

interface RecordingTabsProps {
  active: RecordingTab;
  onChange: (tab: RecordingTab) => void;
}

/**
 * RecordingTabs — segmented control switching between the three side panels
 * (Live Transcript, AI Summary, Notes).
 */
export const RecordingTabs: React.FC<RecordingTabsProps> = ({
  active,
  onChange,
}) => {
  const { t } = useTranslation();

  const tabs: { id: RecordingTab; label: string; icon: typeof FileText }[] = [
    { id: "transcript", label: t("recording.tabs.transcript"), icon: FileText },
    { id: "summary", label: t("recording.tabs.summary"), icon: Sparkles },
    { id: "notes", label: t("recording.tabs.notes"), icon: StickyNote },
  ];

  return (
    <div className="inline-flex p-1 bg-mid-gray/10 rounded-lg gap-1">
      {tabs.map((tab) => {
        const Icon = tab.icon;
        const isActive = active === tab.id;
        return (
          <button
            key={tab.id}
            type="button"
            onClick={() => onChange(tab.id)}
            className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm font-medium transition-colors ${
              isActive
                ? "bg-background text-logo-primary shadow-sm"
                : "text-text/70 hover:text-text"
            }`}
          >
            <Icon width={14} height={14} />
            <span>{tab.label}</span>
          </button>
        );
      })}
    </div>
  );
};

export default RecordingTabs;
