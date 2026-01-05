import React from "react";
import { useTranslation } from "react-i18next";
import { Video } from "lucide-react";
import { MeetingHistory } from "../../meeting";

export const HistorySettings: React.FC = () => {
  const { t } = useTranslation();

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      {/* Meeting History Section */}
      <div className="space-y-2">
        <div className="px-4 flex items-center gap-2">
          <Video className="w-4 h-4 text-mid-gray" />
          <h2 className="text-xs font-medium text-mid-gray uppercase tracking-wide">
            {t("meeting.history.title", "Meeting Recordings")}
          </h2>
        </div>
        <div className="bg-background border border-mid-gray/20 rounded-lg overflow-visible">
          <MeetingHistory />
        </div>
      </div>
    </div>
  );
};
