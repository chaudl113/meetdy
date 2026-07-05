import React, { useMemo, useCallback } from "react";
import { useTranslation } from "react-i18next";
import {
  Cog,
  FlaskConical,
  History,
  Info,
  Settings,
  Sparkles,
  Video,
  Boxes,
} from "lucide-react";
import MeetdyIcon from "./icons/MeetdyIcon";
import { useSettingsStore } from "../stores/settingsStore";
import { useMeetingStore } from "../stores/meetingStore";

export type SidebarSection = keyof typeof SECTIONS_CONFIG;

interface IconProps {
  width?: number | string;
  height?: number | string;
  size?: number | string;
  className?: string;
  [key: string]: any;
}

interface SectionConfig {
  labelKey: string;
  icon: React.ComponentType<IconProps>;
  enabled: (settings: any) => boolean;
  badge?: "new" | "count";
}

export const SECTIONS_CONFIG = {
  general: {
    labelKey: "sidebar.general",
    icon: Settings,
    enabled: () => true,
  },
  meeting: {
    labelKey: "sidebar.meeting",
    icon: Video,
    enabled: () => true,
    badge: "new",
  },
  models: {
    labelKey: "sidebar.models",
    icon: Boxes,
    enabled: () => true,
  },
  advanced: {
    labelKey: "sidebar.advanced",
    icon: Cog,
    enabled: () => true,
  },
  postprocessing: {
    labelKey: "sidebar.postProcessing",
    icon: Sparkles,
    enabled: () => true,
  },
  recordings: {
    labelKey: "sidebar.history",
    icon: History,
    enabled: () => true,
    badge: "count",
  },
  debug: {
    labelKey: "sidebar.debug",
    icon: FlaskConical,
    enabled: (settings) => settings?.debug_mode ?? false,
  },
  about: {
    labelKey: "sidebar.about",
    icon: Info,
    enabled: () => true,
  },
} as const satisfies Record<string, SectionConfig>;

interface SidebarProps {
  activeSection: SidebarSection;
  onSectionChange: (section: SidebarSection) => void;
}

export const Sidebar: React.FC<SidebarProps> = ({
  activeSection,
  onSectionChange,
}) => {
  const { t } = useTranslation();
  const settings = useSettingsStore((s) => s.settings);
  const sessions = useMeetingStore((s) => s.sessions);

  const availableSections = useMemo(
    () =>
      Object.entries(SECTIONS_CONFIG)
        .filter(([_, config]) => config.enabled(settings))
        .map(([id, config]) => ({ id: id as SidebarSection, ...config })),
    [settings],
  );

  const historyCount = sessions?.length ?? 0;

  const handleSectionClick = useCallback(
    (sectionId: SidebarSection) => {
      onSectionChange(sectionId);
    },
    [onSectionChange],
  );

  return (
    <div className="flex flex-col w-56 h-full border-r border-mid-gray/20 bg-background">
      {/* Header: logo + app name + version + pro badge */}
      <div className="flex items-center gap-3 px-4 py-4">
        <div className="w-12 h-12 rounded-xl bg-logo-primary/15 flex items-center justify-center shrink-0">
          <MeetdyIcon width={28} height={28} />
        </div>
        <div className="flex flex-col min-w-0">
          <span className="text-base font-semibold leading-tight truncate">
            {"Meetdy"}
          </span>
          <div className="flex items-center gap-1.5 mt-0.5">
            <span className="text-xs text-text/60">v0.7.0</span>
            <span className="text-[10px] font-semibold px-1.5 py-0.5 rounded-md bg-logo-primary/15 text-logo-primary">
              {"Pro"}
            </span>
          </div>
        </div>
      </div>

      {/* Menu items */}
      <nav className="flex-1 flex flex-col gap-1 px-2 overflow-y-auto">
        {availableSections.map((section) => {
          const Icon = section.icon;
          const isActive = activeSection === section.id;
          const badge = "badge" in section ? section.badge : undefined;
          const showCountBadge = badge === "count" && historyCount > 0;
          const showNewBadge = badge === "new";

          return (
            <button
              key={section.id}
              type="button"
              onClick={() => handleSectionClick(section.id)}
              className={`flex items-center gap-3 w-full px-2.5 py-2 rounded-lg cursor-pointer transition-colors text-left ${
                isActive
                  ? "bg-logo-primary/15 text-logo-primary"
                  : "text-text/80 hover:bg-mid-gray/10"
              }`}
            >
              <span
                className={`flex items-center justify-center w-7 h-7 rounded-md shrink-0 ${
                  isActive ? "text-logo-primary" : "text-text/70"
                }`}
              >
                <Icon width={18} height={18} />
              </span>
              <span
                className="flex-1 text-sm font-medium truncate"
                title={t(section.labelKey)}
              >
                {t(section.labelKey)}
              </span>
              {showNewBadge && (
                <span className="text-[10px] font-semibold px-1.5 py-0.5 rounded-md bg-logo-primary/15 text-logo-primary">
                  {t("sidebar.newBadge")}
                </span>
              )}
              {showCountBadge && (
                <span className="text-[10px] font-semibold min-w-[20px] h-5 px-1.5 rounded-full bg-logo-primary/15 text-logo-primary flex items-center justify-center">
                  {historyCount}
                </span>
              )}
            </button>
          );
        })}
      </nav>
    </div>
  );
};
