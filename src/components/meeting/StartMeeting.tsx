import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  AlertCircle,
  Check,
  CheckSquare,
  ChevronDown,
  FileText,
  LayoutTemplate,
  Lightbulb,
  Mic,
  Monitor,
  Play,
  Sparkles,
  StickyNote,
  Upload,
} from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { useShallow } from "zustand/react/shallow";
import { useMeetingStore } from "../../stores/meetingStore";
import { useRecordingConfigStore } from "../../stores/recordingConfigStore";
import { type AudioSourceType, commands, type MeetingSession, type MeetingTemplate } from "@/bindings";

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

export const StartMeeting: React.FC = () => {
  const { t } = useTranslation();
  const { startMeeting, sessionStatus, isLoading, error, setCurrentSession, setSessionStatus, fetchSessions } = useMeetingStore(
    useShallow((s) => ({
      startMeeting: s.startMeeting,
      sessionStatus: s.sessionStatus,
      isLoading: s.isLoading,
      error: s.error,
      setCurrentSession: s.setCurrentSession,
      setSessionStatus: s.setSessionStatus,
      fetchSessions: s.fetchSessions,
    })),
  );

  const {
    audioSource,
    setAudioSource,
    meetingTitle,
    setMeetingTitle,
    participants,
    setParticipants,
    tags,
    setTags,
  } = useRecordingConfigStore();

  const [templates, setTemplates] = useState<MeetingTemplate[]>([]);
  const [selectedTemplateId, setSelectedTemplateId] = useState<string | null>(null);
  const [isImporting, setIsImporting] = useState(false);
  const [importError, setImportError] = useState<string | null>(null);

  useEffect(() => {
    commands.listMeetingTemplates().then((r) => {
      if (r.status === "ok") setTemplates(r.data);
    });
  }, []);

  const handleTemplateSelect = (tpl: MeetingTemplate | null) => {
    if (!tpl) {
      setSelectedTemplateId(null);
      return;
    }
    setSelectedTemplateId(tpl.id);
    // Apply audio source from template
    const audioMap: Record<string, AudioSourceType> = {
      microphone_only: "microphone_only",
      system_only: "system_only",
      mixed: "mixed",
    };
    if (audioMap[tpl.audio_source]) setAudioSource(audioMap[tpl.audio_source]);
  };

  const isRecording = sessionStatus === "recording";
  const startDisabled = isLoading || isRecording;

  const handleStart = async () => {
    await startMeeting(audioSource, selectedTemplateId ?? undefined);
  };

  const handleImportAudio = async () => {
    setImportError(null);
    const filePath = await open({
      title: t("startMeeting.importAudio", "Import Audio File"),
      multiple: false,
      filters: [
        {
          name: t("startMeeting.importAudioDesc", "Audio Files"),
          extensions: ["wav", "mp3", "m4a", "flac", "ogg"],
        },
      ],
    });
    if (!filePath) return;
    setIsImporting(true);
    try {
      const result = await commands.importMeetingAudio(
        filePath as string,
        meetingTitle || null,
      );
      if (result.status === "ok") {
        const session: MeetingSession = result.data;
        await fetchSessions();
        setCurrentSession(session);
        setSessionStatus("processing");
      } else {
        setImportError(result.error);
      }
    } catch (err) {
      setImportError(err instanceof Error ? err.message : "Import failed");
    } finally {
      setIsImporting(false);
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

  const tips = [
    t("startMeeting.tips.tip1"),
    t("startMeeting.tips.tip2"),
    t("startMeeting.tips.tip3"),
  ];

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
          {/* 0. Templates */}
          {templates.length > 0 && (
            <Card>
              <div className="flex items-center gap-2 mb-3">
                <LayoutTemplate width={16} height={16} className="text-text/60" />
                <span className="text-sm font-semibold">
                  {t("startMeeting.section.templates", "Quick Templates")}
                </span>
              </div>
              <div className="flex flex-col gap-2">
                {templates.map((tpl) => {
                  const isSelected = selectedTemplateId === tpl.id;
                  return (
                    <div
                      key={tpl.id}
                      className={`rounded-lg border transition-all ${
                        isSelected
                          ? "border-logo-primary bg-logo-primary/5"
                          : "border-mid-gray/20"
                      }`}
                    >
                      <button
                        type="button"
                        onClick={() => handleTemplateSelect(isSelected ? null : tpl)}
                        className="w-full flex items-center justify-between gap-3 px-3 py-2.5 text-left"
                      >
                        <div className="flex items-center gap-2">
                          {isSelected && (
                            <Check width={13} height={13} strokeWidth={3} className="text-logo-primary shrink-0" />
                          )}
                          <span className={`text-sm font-medium ${isSelected ? "text-logo-primary" : ""}`}>
                            {tpl.name}
                          </span>
                        </div>
                        <ChevronDown
                          width={14} height={14}
                          className={`text-text/40 transition-transform ${isSelected ? "rotate-180" : ""}`}
                        />
                      </button>
                    </div>
                  );
                })}
              </div>
            </Card>
          )}

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
            {audioSource !== "microphone_only" && (
              <div className="mt-4 rounded-lg border border-yellow-500/25 bg-yellow-500/10 px-3 py-2 text-xs leading-relaxed text-yellow-700 dark:text-yellow-400">
                {t(
                  "startMeeting.source.systemAudioWarning",
                  "System audio capture can conflict with Bluetooth headsets on macOS and may make playback hard to hear. If that happens, switch to Microphone Only.",
                )}
              </div>
            )}

            {/* Action buttons */}
            <div className="mt-6 flex flex-col gap-2">
              <button
                type="button"
                disabled={startDisabled}
                onClick={handleStart}
                className={`w-full flex items-center justify-center gap-2 px-6 py-3 rounded-xl font-semibold text-white transition-colors ${
                  startDisabled
                    ? "bg-logo-primary/50 cursor-not-allowed"
                    : "bg-logo-primary hover:bg-logo-primary/90"
                }`}
              >
                <Play width={18} height={18} fill="currentColor" />
                <span>{t("startMeeting.startRecording")}</span>
              </button>
              <button
                type="button"
                disabled={isImporting || isLoading}
                onClick={handleImportAudio}
                className="w-full flex items-center justify-center gap-2 px-6 py-3 rounded-xl font-semibold border border-mid-gray/30 text-text/80 hover:bg-mid-gray/10 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
              >
                <Upload width={18} height={18} />
                <span>
                  {isImporting
                    ? t("common.loading", "Loading...")
                    : t("startMeeting.importAudio", "Import Audio File")}
                </span>
              </button>
            </div>
            {(error || importError) && (
              <div className="mt-3 flex items-start gap-2 rounded-lg border border-red-500/25 bg-red-500/10 px-3 py-2 text-sm text-red-600 dark:text-red-400">
                <AlertCircle width={16} height={16} className="mt-0.5 shrink-0" />
                <span className="break-words">{importError ?? error}</span>
              </div>
            )}
          </Card>
        </div>

        {/* Right column (1/3) */}
        <div className="flex flex-col gap-4">
          {/* 3. Meeting Info */}
          <Card>
            <SectionHeader
              index={2}
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
                <input
                  type="text"
                  value={tags}
                  onChange={(e) => setTags(e.target.value)}
                  placeholder={t("startMeeting.tagsPlaceholder")}
                  className="w-full bg-background border border-mid-gray/30 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-logo-primary"
                />
              </div>
            </div>
          </Card>

          {/* 4. What will be generated */}
          <Card>
            <SectionHeader
              index={3}
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
