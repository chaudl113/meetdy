import React from "react";
import { MeetingSummary } from "./MeetingSummary";

interface SummarySectionProps {
  sessionId: string;
  summary: string | null;
  hasSummary: boolean;
  hasTranscript: boolean;
  onSummaryGenerated: (summary: string) => void;
}

export const SummarySection: React.FC<SummarySectionProps> = ({
  sessionId,
  summary,
  hasSummary,
  hasTranscript,
  onSummaryGenerated,
}) => {
  return (
    <MeetingSummary
      sessionId={sessionId}
      summary={summary}
      hasSummary={hasSummary}
      hasTranscript={hasTranscript}
      onSummaryGenerated={onSummaryGenerated}
    />
  );
};

export default SummarySection;
