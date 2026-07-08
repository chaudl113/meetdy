import React, { useEffect, useRef, useState } from "react";
import { Virtuoso, type VirtuosoHandle } from "react-virtuoso";
import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";
import { FileText } from "lucide-react";
import { useMeetingStore } from "../../../stores/meetingStore";
import { useSettings } from "../../../hooks/useSettings";
import type { Participant } from "@/bindings";
import { SpeakerSelector } from "./SpeakerSelector";
import { SpeakerSegment } from "./SpeakerSegment";
import { UNKNOWN_SPEAKER_COLOR, useSpeakerColors, type SpeakerColor } from "../../../hooks/useSpeakerColors";

const formatOffset = (seconds: number): string => {
  const safe = Math.max(0, Math.floor(seconds));
  const m = Math.floor(safe / 60);
  const s = safe % 60;
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
};

// Suppress unused warning — kept for potential future use
void formatOffset;

interface LiveSegment {
  text: string;
  offset: number;
  speakerId: string | null;
}

interface LiveVirtuosoListProps {
  segments: LiveSegment[];
  speakerColors: Record<string, SpeakerColor>;
  participantMap: Record<string, string>;
}

const LiveVirtuosoList: React.FC<LiveVirtuosoListProps> = ({
  segments,
  speakerColors,
  participantMap,
}) => {
  const virtuosoRef = useRef<VirtuosoHandle>(null);

  // Follow output — scroll to bottom when new segments arrive
  useEffect(() => {
    if (segments.length > 0) {
      virtuosoRef.current?.scrollToIndex({
        index: segments.length - 1,
        behavior: "smooth",
      });
    }
  }, [segments.length]);

  return (
    <Virtuoso
      ref={virtuosoRef}
      data={segments}
      style={{ height: "300px" }}
      followOutput="smooth"
      itemContent={(index, segment) => {
        const speakerId = segment.speakerId ?? null;
        const prevSpeakerId =
          index > 0 ? (segments[index - 1].speakerId ?? null) : undefined;
        const showLabel = speakerId !== prevSpeakerId;
        const color = speakerId
          ? (speakerColors[speakerId] ?? UNKNOWN_SPEAKER_COLOR)
          : UNKNOWN_SPEAKER_COLOR;
        return (
          <SpeakerSegment
            key={`${index}-${segment.text.slice(0, 24)}`}
            text={segment.text}
            startMs={segment.offset * 1000}
            speakerName={speakerId ? (participantMap[speakerId] ?? null) : null}
            color={color}
            showSpeakerLabel={showLabel}
          />
        );
      }}
    />
  );
};

interface LiveTranscriptPanelProps {
  sessionId: string;
  participants: Participant[];
  activeSpeakerId: string | null;
  onActiveSpeakerChange: (id: string | null) => void;
  onParticipantAdded: (p: Participant) => void;
}

export const LiveTranscriptPanel: React.FC<LiveTranscriptPanelProps> = ({
  sessionId,
  participants,
  activeSpeakerId,
  onActiveSpeakerChange,
  onParticipantAdded,
}) => {
  const { t } = useTranslation();
  const { sessionStatus, liveTranscript, liveTranscriptSegments } =
    useMeetingStore(
      useShallow((s) => ({
        sessionStatus: s.sessionStatus,
        liveTranscript: s.liveTranscript,
        liveTranscriptSegments: s.liveTranscriptSegments,
      })),
    );
  const { getSetting } = useSettings();
  const isFunasr = (getSetting("meeting_stt_engine") ?? "whisper") === "funasr";
  const diarizationEnabled = getSetting("diarization_enabled") ?? false;

  const speakerColors = useSpeakerColors(participants);
  const participantMap = Object.fromEntries(participants.map((p) => [p.id, p.name]));

  // Speaker hint: show a brief toast-like hint when active speaker changes
  const [speakerHint, setSpeakerHint] = useState<string | null>(null);
  const hintTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (!diarizationEnabled || !activeSpeakerId) return;
    const name = participantMap[activeSpeakerId];
    if (!name) return;

    setSpeakerHint(name);
    if (hintTimerRef.current) clearTimeout(hintTimerRef.current);
    hintTimerRef.current = setTimeout(() => setSpeakerHint(null), 3000);
  }, [activeSpeakerId]); // eslint-disable-line react-hooks/exhaustive-deps

  const speakerBar = (
    <SpeakerSelector
      sessionId={sessionId}
      participants={participants}
      activeSpeakerId={activeSpeakerId}
      onActiveSpeakerChange={onActiveSpeakerChange}
      onParticipantAdded={onParticipantAdded}
    />
  );

  if (liveTranscript.trim() || liveTranscriptSegments.length) {
    const segments = liveTranscriptSegments.length
      ? liveTranscriptSegments
      : [{ text: liveTranscript, offset: 0, speakerId: null }];

    return (
      <div className="flex flex-col gap-3">
        {speakerBar}

        {speakerHint && (
          <div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-logo-primary/10 text-logo-primary text-sm animate-pulse">
            <span>🎤</span>
            <span>{t("recording.speaker.label")}: <strong>{speakerHint}</strong></span>
          </div>
        )}

        <LiveVirtuosoList
          segments={segments}
          speakerColors={speakerColors}
          participantMap={participantMap}
        />

        {sessionStatus === "recording" && (
          <div className="flex items-center gap-2 text-sm text-text/50">
            <span className="inline-block w-2 h-2 rounded-full bg-blue-500 animate-pulse" />
            {t("recording.transcript.listening", "Listening…")}
          </div>
        )}
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3">
      {speakerBar}
      <div className="flex flex-col items-center justify-center text-center py-12 px-6">
        <FileText className="text-text/30 mb-3" width={40} height={40} />
        <h3 className="text-base font-semibold mb-1">
          {isFunasr
            ? t("recording.transcript.funasrEmptyTitle", "Waiting for speech")
            : t("recording.transcript.emptyTitle")}
        </h3>
        <p className="text-sm text-text/60 max-w-sm">
          {isFunasr
            ? t(
                "recording.transcript.funasrEmptyMessage",
                "FunASR transcribes local audio chunks while recording and refreshes the final transcript after End Meeting.",
              )
            : t("recording.transcript.emptyMessage")}
        </p>
      </div>
    </div>
  );
};

export default LiveTranscriptPanel;
