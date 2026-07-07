import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";
import { Copy, Check, FileText } from "lucide-react";
import { useMeetingStore } from "../../stores/meetingStore";
import { commands, type Participant } from "@/bindings";
import { SpeakerSegment } from "./recording/SpeakerSegment";
import {
  useSpeakerColors,
  UNKNOWN_SPEAKER_COLOR,
} from "../../hooks/useSpeakerColors";
import { TTSButton } from "./TTSButton";

// Inline type until bindings are regenerated
interface TranscriptSegmentLocal {
  id: string;
  meeting_id: string;
  start_ms: number;
  end_ms: number;
  text: string;
  speaker_id: string | null;
  sequence: number;
  created_at: number;
}

export const MeetingTranscriptDisplay: React.FC = () => {
  const { t } = useTranslation();
  const { currentSession, sessionStatus } = useMeetingStore(
    useShallow((s) => ({
      currentSession: s.currentSession,
      sessionStatus: s.sessionStatus,
    })),
  );
  const [transcript, setTranscript] = useState<string | null>(null);
  const [segments, setSegments] = useState<TranscriptSegmentLocal[]>([]);
  const [participants, setParticipants] = useState<Participant[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [isCopied, setIsCopied] = useState(false);

  const speakerColors = useSpeakerColors(participants);
  const participantMap = Object.fromEntries(
    participants.map((p) => [p.id, p.name]),
  );

  useEffect(() => {
    if (!currentSession?.id || sessionStatus !== "completed") {
      setTranscript(null);
      setSegments([]);
      setParticipants([]);
      return;
    }

    const fetchAll = async () => {
      setIsLoading(true);
      try {
        const [transcriptResult, segmentsResult, participantsResult] =
          await Promise.allSettled([
            commands.getMeetingTranscript(currentSession.id),
            (commands as any).getMeetingTranscriptSegments(
              currentSession.id,
            ) as Promise<
              | { status: "ok"; data: TranscriptSegmentLocal[] }
              | { status: "error"; error: string }
            >,
            commands.listMeetingParticipants(currentSession.id),
          ]);

        if (
          transcriptResult.status === "fulfilled" &&
          transcriptResult.value.status === "ok"
        ) {
          setTranscript(transcriptResult.value.data);
        }
        if (
          segmentsResult.status === "fulfilled" &&
          segmentsResult.value.status === "ok"
        ) {
          setSegments(segmentsResult.value.data);
        }
        if (
          participantsResult.status === "fulfilled" &&
          participantsResult.value.status === "ok"
        ) {
          setParticipants(participantsResult.value.data);
        }
      } catch (err) {
        console.error("Failed to fetch transcript data:", err);
      } finally {
        setIsLoading(false);
      }
    };

    fetchAll();
  }, [currentSession?.id, currentSession?.transcript_path, sessionStatus]);

  const handleCopy = async () => {
    const text =
      segments.length > 0
        ? segments.map((s) => s.text).join("\n")
        : transcript;
    if (!text) return;
    try {
      await navigator.clipboard.writeText(text);
      setIsCopied(true);
      setTimeout(() => setIsCopied(false), 2000);
    } catch (err) {
      console.error("Failed to copy:", err);
    }
  };

  if (sessionStatus !== "completed") return null;

  const hasContent = segments.length > 0 || transcript;

  return (
    <div className="mt-4 overflow-hidden rounded-xl border border-mid-gray/20 bg-background shadow-sm">
      <div className="flex items-center justify-between px-4 py-3 border-b border-mid-gray/15 bg-mid-gray/[0.04]">
        <div className="flex items-center gap-2 text-sm font-semibold text-text">
          <FileText size={16} />
          {t("meeting.transcript", "Transcript")}
        </div>
        {hasContent && (
          <div className="flex items-center gap-1">
            <TTSButton
              getText={() =>
                segments.length > 0
                  ? segments.map((s) => s.text).join(" ")
                  : transcript ?? ""
              }
            />
            <button
              onClick={handleCopy}
              className="flex items-center gap-1.5 px-2.5 py-1.5 text-xs font-medium text-text/60 hover:text-text hover:bg-mid-gray/10 rounded-lg transition-colors"
              title={t("meeting.copyTranscript", "Copy transcript")}
            >
              {isCopied ? (
                <>
                  <Check size={14} className="text-green-500" />
                  {t("common.copied", "Copied!")}
                </>
              ) : (
                <>
                  <Copy size={14} />
                  {t("common.copy", "Copy")}
                </>
              )}
            </button>
          </div>
        )}
      </div>

      <div className="p-5">
        {isLoading ? (
          <div className="flex items-center gap-2 text-sm text-text/60">
            <span className="inline-flex h-4 w-4 rounded-full border-2 border-mid-gray border-t-transparent animate-spin" />
            {t("common.loading", "Loading...")}
          </div>
        ) : segments.length > 0 ? (
          <div className="flex flex-col">
            {segments.map((seg, index) => {
              const prevSpeakerId =
                index > 0 ? segments[index - 1].speaker_id : undefined;
              const showLabel = seg.speaker_id !== prevSpeakerId;
              const color = seg.speaker_id
                ? (speakerColors[seg.speaker_id] ?? UNKNOWN_SPEAKER_COLOR)
                : UNKNOWN_SPEAKER_COLOR;
              return (
                <SpeakerSegment
                  key={seg.id}
                  text={seg.text}
                  startMs={seg.start_ms}
                  speakerName={
                    seg.speaker_id ? participantMap[seg.speaker_id] : null
                  }
                  color={color}
                  showSpeakerLabel={showLabel}
                />
              );
            })}
          </div>
        ) : transcript ? (
          <p className="text-[15px] text-text whitespace-pre-wrap leading-7">
            {transcript}
          </p>
        ) : (
          <p className="text-sm text-text/50 italic">
            {t("meeting.noTranscript", "No transcript available")}
          </p>
        )}
      </div>
    </div>
  );
};

export default MeetingTranscriptDisplay;
