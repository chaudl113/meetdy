import React from "react";
import { ProgressBar, ProgressData } from "../shared";
import type { DownloadProgress, DownloadStats } from "@/stores/modelEventStore";

interface DownloadProgressDisplayProps {
  downloadProgress: Record<string, DownloadProgress>;
  downloadStats: Record<string, DownloadStats>;
  className?: string;
}

const DownloadProgressDisplay: React.FC<DownloadProgressDisplayProps> = ({
  downloadProgress,
  downloadStats,
  className = "",
}) => {
  const entries = Object.entries(downloadProgress);
  if (entries.length === 0) {
    return null;
  }

  const progressData: ProgressData[] = entries.map(([modelId, progress]) => {
    const stats = downloadStats[modelId];
    return {
      id: modelId,
      percentage: progress.percentage,
      speed: stats?.speed,
    };
  });

  return (
    <ProgressBar
      progress={progressData}
      className={className}
      showSpeed={entries.length === 1}
      size="medium"
    />
  );
};

export default DownloadProgressDisplay;
