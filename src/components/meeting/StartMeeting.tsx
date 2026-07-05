import React, { useMemo } from "react";
import { useTranslation } from "react-i18next";
import {
  Monitor,
  Mic,
  Check,
  ChevronDown,
  FileText,
  Sparkles,
  CheckSquare,
  StickyNote,
  Lightbulb,
  Calendar,
  Play,
} from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { useMeetingStore } from "../../stores/meetingStore";
import {
  useRecordingConfigStore,
  type RecordingQuality,
} from "../../stores/recordingConfigStore";
import { useSettings } from "../../hooks/useSettings";
import { LANGUAGES } from "../../lib/constants/languages";
import type { AudioSourceType } from "@/bindings";

interface SourceOption {
  id: AudioSourceType;
  labelKey: string;
  descKey: string;
  renderIcon: () => React.ReactNode;
}

const SOURCE_OPTIONS: SourceOption[] = [
  {
    id: "system_only",
    labelKey: "startMeeting.source.systemAudio.label",
    descKey: "startMeeting.source.systemAudio.description",
    renderIcon: () => <Monitor width={28} height={28} />,
  },
  {
    id: "microphone_only",
    labelKey: "startMeeting.source.microphone.label",
    descKey: "startMeeting.source.microphone.description",
    renderIcon: () => <Mic width={28} height={28} />,
  },
  {
    id: "mixed",
    labelKey: "startMeeting.source.mixed.label",
    descKey: "startMeeting.source.mixed.description",
    renderIcon: () => (
      <div className="flex items-center gap-1">
        <Monitor width={24} height={24} />
        <span className="text-text/60">+</span>
        <Mic width={24} height={24} />
      </div>
    ),
  },
];

const SectionHeader: React.FC<{ index: number; title: string; suffix?: string }> = ({
  index,
  title,
  suffix,
}) => (
  <div className="flex items-baseline gap-1">
    <span className="text-logo-primary font-semibold">
      {index}. {title}
    </span>
    {suffix && <span className="text-text/50 text-sm">{suffix}</span>}
  </div>
);

const Card: React.FC<{ children: React.ReactNode; className?: string }> = ({
  children,
  className = "",
}) => (
  <div
    className={`bg-background border border-mid-gray/20 rounded-xl p-5 ${className}`}
  >
    {children}
  </div>
);

const Toggle: React.FC<{
  checked: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
}> = ({ checked, onChange, disabled }) => (
  <button
    type="button"
    role="switch"
    aria-checked={checked}
    disabled={disabled}
    onClick={() => onChange(!checked)}
    className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
      checked ? "bg-logo-primary" : "bg-mid-gray/30"
    } ${disabled ? "opacity-50 cursor-not-allowed" : "cursor-pointer"}`}
  >
    <span
      className={`inline-block h-5 w-5 transform rounded-full bg-white transition-transform ${
        checked ? "translate-x-5" : "translate-x-0.5"
      }`}
    />
  </button>
);

export const StartMeeting: React.FC = () => {
  const { t } = useTranslation();
  const { startMeeting, sessionStatus, isLoading } = useMeetingStore();
  const { getSetting, updateSetting, isUpdating } = useSettings();

  // Persisted form state lives in `recordingConfigStore` so RecordingView can
  // read the choices made here (quality, save location, toggles, ...).
  const {
    audioSource,
    setAudioSource,
    recordingQuality,
    setRecordingQuality,
    autoTranscribe,
    setAutoTranscribe,
    autoSummary,
    setAutoSummary,
    meetingTitle,
    setMeetingTitle,
    participants,
    setParticipants,
    tags,
    setTags,
    saveLocation,
    setSaveLocation,
  } = useRecordingConfigStore();

  const language = getSetting("selected_language") || "auto";
  const isLanguageUpdating = isUpdating("selected_language");

  const isRecording = sessionStatus === "recording";
  const startDisabled = isLoading || isRecording;

  const handleStart = async () => {
    await startMeeting(audioSource);
  };

  const handleLanguageChange = async (
    e: React.ChangeEvent<HTMLSelectElement>,
  ) => {
    await updateSetting("selected_language", e.target.value);
  };

  const handleChangeSaveLocation = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: t("startMeeting.selectFolderTitle"),
      });
      if (typeof selected === "string" && selected.length > 0) {
        setSaveLocation(selected);
      }
    } catch (err) {
      console.error("Failed to open folder picker:", err);
    }
  };

  const generationItems = [
    {
      icon: FileText,
      titleKey: "startMeeting.generated.transcription.title",
      descKey: "startMeeting.generated.transcription.description",
    },
    {
      icon: Sparkles,
      titleKey: "startMeeting.generated.summary.title",
      descKey: "startMeeting.generated.summary.description",
    },
    {
      icon: CheckSquare,
      titleKey: "startMeeting.generated.actionItems.title",
      descKey: "startMeeting.generated.actionItems.description",
    },
    {
      icon: StickyNote,
      titleKey: "startMeeting.generated.notes.title",
      descKey: "startMeeting.generated.notes.description",
    },
  ];

  const tips = useMemo(() => [
    t("startMeeting.tips.tip1"),
    t("startMeeting.tips.tip2"),
    t("startMeeting.tips.tip3"),
  ], [t]);

  return (
    <div className="w-full max-w-[1200px]">
      {/* Page header */}
      <div className="mb-6">
        <h1 className="text-2xl font-bold mb-1">
          {t("startMeeting.title")}
        </h1>
        <p className="text-text/60 text-sm">{t("startMeeting.subtitle")}</p>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        {/* Left column (2/3) */}
        <div className="lg:col-span-2 flex flex-col gap-4">
          {/* 1. Meeting Source */}
          <Card>
            <SectionHeader index={1} title={t("startMeeting.section.source")} />
            <p className="text-sm text-text/60 mt-1 mb-4">
              {t("startMeeting.sourceHint")}
            </p>
            <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
              {SOURCE_OPTIONS.map((opt) => {
                const isSelected = audioSource === opt.id;
                return (
                  <button
                    key={opt.id}
                    type="button"
                    onClick={() => setAudioSource(opt.id)}
                    className={`relative rounded-xl border p-4 text-center transition-all ${
                      isSelected
                        ? "border-logo-primary bg-logo-primary/5 ring-1 ring-logo-primary/40"
                        : "border-mid-gray/20 hover:border-logo-primary/40"
                    }`}
                  >
                    {isSelected && (
                      <span className="absolute top-2 right-2 w-5 h-5 rounded-full bg-logo-primary flex items-center justify-center">
                        <Check
                          width={12}
                          height={12}
                          className="text-white"
                          strokeWidth={3}
                        />
                      </span>
                    )}
                    <div
                      className={`flex items-center justify-center h-12 mb-3 ${
                        isSelected ? "text-logo-primary" : "text-text/70"
                      }`}
                    >
                      {opt.renderIcon()}
                    </div>
                    <div
                      className={`text-sm font-semibold mb-1 ${
                        isSelected ? "text-logo-primary" : ""
                      }`}
                    >
                      {t(opt.labelKey)}
                    </div>
                    <div className="text-xs text-text/60 leading-snug">
                      {t(opt.descKey)}
                    </div>
                  </button>
                );
              })}
            </div>
          </Card>

          {/* 2. Recording Settings */}
          <Card>
            <SectionHeader
              index={2}
              title={t("startMeeting.section.settings")}
            />

            <div className="mt-4 flex flex-col gap-4">
              {/* Recording Quality */}
              <div className="flex items-center justify-between gap-4">
                <label className="text-sm font-medium">
                  {t("startMeeting.recordingQuality")}
                </label>
                <div className="relative w-64">
                  <select
                    value={recordingQuality}
                    onChange={(e) =>
                      setRecordingQuality(e.target.value as RecordingQuality)
                    }
                    className="w-full appearance-none bg-background border border-mid-gray/30 rounded-lg px-3 py-2 text-sm pr-8 focus:outline-none focus:border-logo-primary cursor-pointer"
                  >
                    <option value="low">{t("startMeeting.quality.low")}</option>
                    <option value="medium">
                      {t("startMeeting.quality.medium")}
                    </option>
                    <option value="high">
                      {t("startMeeting.quality.highRecommended")}
                    </option>
                  </select>
                  <ChevronDown
                    width={16}
                    height={16}
                    className="absolute right-2 top-1/2 -translate-y-1/2 pointer-events-none text-text/60"
                  />
                </div>
              </div>

              {/* Language */}
              <div className="flex items-center justify-between gap-4">
                <label className="text-sm font-medium">
                  {t("startMeeting.language")}
                </label>
                <div className="relative w-64">
                  <select
                    value={language}
                    onChange={handleLanguageChange}
                    disabled={isLanguageUpdating}
                    className={`w-full appearance-none bg-background border border-mid-gray/30 rounded-lg px-3 py-2 text-sm pr-8 focus:outline-none focus:border-logo-primary ${
                      isLanguageUpdating
                        ? "opacity-50 cursor-not-allowed"
                        : "cursor-pointer"
                    }`}
                  >
                    {LANGUAGES.map((lang) => (
                      <option key={lang.value} value={lang.value}>
                        {lang.value === "auto"
                          ? t("startMeeting.autoDetect")
                          : lang.label}
                      </option>
                    ))}
                  </select>
                  <ChevronDown
                    width={16}
                    height={16}
                    className="absolute right-2 top-1/2 -translate-y-1/2 pointer-events-none text-text/60"
                  />
                </div>
              </div>

              {/* Save Location */}
              <div className="flex items-center justify-between gap-4">
                <label className="text-sm font-medium">
                  {t("startMeeting.saveLocation")}
                </label>
                <div className="flex gap-2 w-64">
                  <input
                    readOnly
                    value={saveLocation}
                    title={saveLocation}
                    className="flex-1 min-w-0 bg-background border border-mid-gray/30 rounded-lg px-3 py-2 text-sm focus:outline-none"
                  />
                  <button
                    type="button"
                    onClick={handleChangeSaveLocation}
                    className="px-3 py-2 border border-mid-gray/30 rounded-lg text-sm hover:bg-mid-gray/10 cursor-pointer shrink-0"
                  >
                    {t("startMeeting.change")}
                  </button>
                </div>
              </div>

              {/* Auto Transcribe */}
              <div className="flex items-center justify-between gap-4 pt-2 border-t border-mid-gray/10">
                <div>
                  <div className="text-sm font-medium">
                    {t("startMeeting.autoTranscribe.title")}
                  </div>
                  <div className="text-xs text-text/60">
                    {t("startMeeting.autoTranscribe.description")}
                  </div>
                </div>
                <Toggle checked={autoTranscribe} onChange={setAutoTranscribe} />
              </div>

              {/* Auto Summary */}
              <div className="flex items-center justify-between gap-4">
                <div>
                  <div className="text-sm font-medium">
                    {t("startMeeting.autoSummary.title")}
                  </div>
                  <div className="text-xs text-text/60">
                    {t("startMeeting.autoSummary.description")}
                  </div>
                </div>
                <Toggle checked={autoSummary} onChange={setAutoSummary} />
              </div>
            </div>

            {/* Action buttons */}
            <div className="mt-6 grid grid-cols-1 md:grid-cols-[1fr_auto] gap-3">
              <button
                type="button"
                disabled={startDisabled}
                onClick={handleStart}
                className={`flex items-center justify-center gap-2 px-6 py-3 rounded-xl font-semibold text-white transition-colors ${
                  startDisabled
                    ? "bg-logo-primary/50 cursor-not-allowed"
                    : "bg-logo-primary hover:bg-logo-primary/90"
                }`}
              >
                <Play width={18} height={18} fill="currentColor" />
                <span>{t("startMeeting.startRecording")}</span>
                <span className="ml-2 flex items-center gap-1 text-xs bg-white/20 px-2 py-0.5 rounded">
                  <kbd>{"\u2318"}</kbd>
                  <kbd>R</kbd>
                </span>
              </button>
              <button
                type="button"
                className="flex items-center justify-center gap-2 px-6 py-3 rounded-xl font-semibold border border-mid-gray/30 hover:bg-mid-gray/10"
              >
                <Calendar width={18} height={18} />
                <span>{t("startMeeting.scheduleMeeting")}</span>
              </button>
            </div>
          </Card>
        </div>

        {/* Right column (1/3) */}
        <div className="flex flex-col gap-4">
          {/* 3. Meeting Info */}
          <Card>
            <SectionHeader
              index={3}
              title={t("startMeeting.section.info")}
              suffix={t("startMeeting.optional")}
            />

            <div className="mt-4 flex flex-col gap-4">
              <div>
                <label className="block text-sm font-medium mb-1">
                  {t("startMeeting.meetingTitle")}
                </label>
                <input
                  type="text"
                  value={meetingTitle}
                  onChange={(e) => setMeetingTitle(e.target.value)}
                  placeholder={t("startMeeting.meetingTitlePlaceholder")}
                  className="w-full bg-background border border-mid-gray/30 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-logo-primary"
                />
              </div>

              <div>
                <label className="block text-sm font-medium mb-1">
                  {t("startMeeting.participants")}{" "}
                  <span className="text-text/50 font-normal">
                    {t("startMeeting.commaSeparated")}
                  </span>
                </label>
                <input
                  type="text"
                  value={participants}
                  onChange={(e) => setParticipants(e.target.value)}
                  placeholder={t("startMeeting.participantsPlaceholder")}
                  className="w-full bg-background border border-mid-gray/30 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-logo-primary"
                />
              </div>

              <div>
                <label className="block text-sm font-medium mb-1">
                  {t("startMeeting.tags")}
                </label>
                <div className="relative">
                  <input
                    type="text"
                    value={tags}
                    onChange={(e) => setTags(e.target.value)}
                    placeholder={t("startMeeting.tagsPlaceholder")}
                    className="w-full bg-background border border-mid-gray/30 rounded-lg px-3 py-2 pr-8 text-sm focus:outline-none focus:border-logo-primary"
                  />
                  <ChevronDown
                    width={16}
                    height={16}
                    className="absolute right-2 top-1/2 -translate-y-1/2 pointer-events-none text-text/60"
                  />
                </div>
              </div>
            </div>
          </Card>

          {/* 4. What will be generated */}
          <Card>
            <SectionHeader
              index={4}
              title={t("startMeeting.section.generated")}
            />
            <ul className="mt-4 flex flex-col gap-3">
              {generationItems.map((item) => {
                const Icon = item.icon;
                return (
                  <li key={item.titleKey} className="flex items-start gap-3">
                    <Icon
                      width={20}
                      height={20}
                      className="text-logo-primary shrink-0 mt-0.5"
                    />
                    <div>
                      <div className="text-sm font-semibold">
                        {t(item.titleKey)}
                      </div>
                      <div className="text-xs text-text/60">
                        {t(item.descKey)}
                      </div>
                    </div>
                  </li>
                );
              })}
            </ul>
          </Card>

          {/* Tips */}
          <Card>
            <div className="flex items-center gap-2 mb-3">
              <Lightbulb
                width={18}
                height={18}
                className="text-logo-primary"
              />
              <span className="font-semibold">{t("startMeeting.tips.title")}</span>
            </div>
            <ul className="flex flex-col gap-2">
              {tips.map((tip, i) => (
                <li
                  key={i}
                  className="text-sm text-text/70 flex items-start gap-2"
                >
                  <span className="text-text/40">•</span>
                  <span>{tip}</span>
                </li>
              ))}
            </ul>
          </Card>
        </div>
      </div>
    </div>
  );
};

export default StartMeeting;
