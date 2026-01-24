import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Clock,
  FileText,
  AlertCircle,
  Loader2,
  ChevronRight,
  Search,
} from "lucide-react";
import { useMeetingStore, formatDuration } from "../../stores/meetingStore";
import { MeetingDetailView } from "./MeetingDetailView";
import { Input } from "../ui/Input";
import type { MeetingSession } from "@/bindings";

/**
 * Formats a Unix timestamp to a localized date/time string
 */
function formatDate(timestamp: number): string {
  return new Date(timestamp * 1000).toLocaleString();
}

/**
 * Status badge component for meeting sessions
 */
const StatusBadge: React.FC<{ status: MeetingSession["status"] }> = ({
  status,
}) => {
  const { t } = useTranslation();

  const statusConfig = {
    idle: { bg: "bg-gray-500/20", text: "text-gray-400", label: "Idle" },
    recording: {
      bg: "bg-red-500/20",
      text: "text-red-400",
      label: t("meeting.status.recording", "Recording"),
    },
    processing: {
      bg: "bg-yellow-500/20",
      text: "text-yellow-400",
      label: t("meeting.status.processing", "Processing"),
    },
    completed: {
      bg: "bg-green-500/20",
      text: "text-green-400",
      label: t("meeting.status.completed", "Completed"),
    },
    failed: {
      bg: "bg-red-500/20",
      text: "text-red-400",
      label: t("meeting.status.failed", "Failed"),
    },
    interrupted: {
      bg: "bg-orange-500/20",
      text: "text-orange-400",
      label: t("meeting.status.interrupted", "Interrupted"),
    },
  };

  const config = statusConfig[status] || statusConfig.idle;

  return (
    <span
      className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${config.bg} ${config.text}`}
    >
      {config.label}
    </span>
  );
};

/**
 * MeetingHistory - Displays a list of past meeting sessions with click to view details
 */
export const MeetingHistory: React.FC = () => {
  const { t } = useTranslation();
  const { sessions, fetchSessions, isLoading } = useMeetingStore();
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedSession, setSelectedSession] = useState<MeetingSession | null>(
    null,
  );

  // Fetch sessions on mount
  useEffect(() => {
    fetchSessions();
  }, [fetchSessions]);

  const handleSessionClick = (session: MeetingSession) => {
    setSelectedSession(session);
  };

  const handleCloseDetail = () => {
    setSelectedSession(null);
  };

  const filteredSessions = sessions.filter((session) => {
    if (!searchQuery) return true;

    const query = searchQuery.toLowerCase();
    const title = (session.title || "").toLowerCase();
    const date = formatDate(session.created_at).toLowerCase();
    const status = (session.status || "").toLowerCase();

    return (
      title.includes(query) || date.includes(query) || status.includes(query)
    );
  });

  if (isLoading && sessions.length === 0) {
    return (
      <div className="flex items-center justify-center py-8 text-mid-gray">
        <Loader2 className="h-5 w-5 animate-spin mr-2" />
        <span>{t("common.loading", "Loading...")}</span>
      </div>
    );
  }

  if (sessions.length === 0) {
    return (
      <div className="text-center py-8 text-mid-gray">
        <FileText className="h-8 w-8 mx-auto mb-2 opacity-50" />
        <p>{t("meeting.history.empty", "No meeting recordings yet")}</p>
      </div>
    );
  }

  return (
    <>
      <div className="p-4 border-b border-mid-gray/20">
        <div className="relative">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-mid-gray" />
          <Input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder={t(
              "settings.history.searchPlaceholder",
              "Search meetings by title, date, or status...",
            )}
            className="pl-9 w-full"
          />
        </div>
      </div>

      <div className="divide-y divide-mid-gray/20">
        {filteredSessions.length === 0 ? (
          <div className="text-center py-8 text-mid-gray">
            <Search className="h-8 w-8 mx-auto mb-2 opacity-50" />
            <p>
              {t(
                "settings.history.noResults",
                "No meetings found matching your search",
              )}
            </p>
          </div>
        ) : (
          filteredSessions.map((session) => (
            <div
              key={session.id}
              onClick={() => handleSessionClick(session)}
              className="flex items-center justify-between px-4 py-3 hover:bg-mid-gray/10 cursor-pointer transition-colors group"
            >
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <h4 className="text-sm font-medium truncate">
                    {session.title}
                  </h4>
                  <StatusBadge status={session.status} />
                </div>
                <div className="flex items-center gap-3 mt-1 text-xs text-mid-gray">
                  <span className="flex items-center gap-1">
                    <Clock className="h-3 w-3" />
                    {formatDate(session.created_at)}
                  </span>
                  {session.duration && (
                    <span>{formatDuration(session.duration)}</span>
                  )}
                </div>
                {session.error_message && (
                  <div className="flex items-center gap-1 mt-1 text-xs text-red-400">
                    <AlertCircle className="h-3 w-3" />
                    <span className="truncate">{session.error_message}</span>
                  </div>
                )}
              </div>
              <ChevronRight className="h-4 w-4 text-mid-gray opacity-0 group-hover:opacity-100 transition-opacity" />
            </div>
          ))
        )}
      </div>

      {/* Detail Modal */}
      {selectedSession && (
        <MeetingDetailView
          session={selectedSession}
          onClose={handleCloseDetail}
        />
      )}
    </>
  );
};

export default MeetingHistory;
