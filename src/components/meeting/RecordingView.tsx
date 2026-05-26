import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { RecordingTopBar } from "./recording/RecordingTopBar";
import { RecordingMetaBar } from "./recording/RecordingMetaBar";
import {
  RecordingTabs,
  type RecordingTab,
} from "./recording/RecordingTabs";
import { LiveTranscriptPanel } from "./recording/LiveTranscriptPanel";
import { AISummaryPanel } from "./recording/AISummaryPanel";
import { NotesPanel } from "./recording/NotesPanel";
import { AudioStatsCard } from "./recording/AudioStatsCard";
import { RecordingInfoCard } from "./recording/RecordingInfoCard";
import { QuickNotesCard } from "./recording/QuickNotesCard";
import { BottomControlsBar } from "./recording/BottomControlsBar";
import { RecordingFooter } from "./recording/RecordingFooter";
import { AddNoteModal } from "./recording/AddNoteModal";
import { useMeetingStore } from "../../stores/meetingStore";

/**
 * RecordingView — main UI shown while a meeting is being recorded.
 *
 * Composition (top → bottom):
 *  - RecordingTopBar     : status pill + timer + Pause/Stop/AddNote
 *  - RecordingMetaBar    : editable title + duration / language / quality
 *  - Two-column area     : left tabs (transcript/summary/notes), right cards
 *  - BottomControlsBar   : mic/system/noise toggles + End Meeting
 *  - RecordingFooter     : "Recording will be saved to ..."
 */
export const RecordingView: React.FC = () => {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<RecordingTab>("transcript");
  const [noteModalOpen, setNoteModalOpen] = useState(false);
  const [noteTimestamp, setNoteTimestamp] = useState(0);

  const { addNote, recordingDuration } = useMeetingStore();

  const openAddNote = () => {
    // Pin the note to the moment the button was clicked, not to the moment
    // the user finishes typing.
    setNoteTimestamp(recordingDuration);
    setNoteModalOpen(true);
  };

  const handleSaveNote = async (content: string) => {
    await addNote(noteTimestamp, content);
  };

  const renderPanel = () => {
    switch (activeTab) {
      case "transcript":
        return <LiveTranscriptPanel />;
      case "summary":
        return <AISummaryPanel />;
      case "notes":
        return <NotesPanel onAddNote={openAddNote} />;
    }
  };

  return (
    <div className="w-full max-w-[1200px] flex flex-col gap-4">
      <RecordingTopBar onAddNote={openAddNote} />
      <RecordingMetaBar />

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        {/* Left column (2/3) — tabs */}
        <div className="lg:col-span-2 bg-background border border-mid-gray/20 rounded-xl p-5 flex flex-col gap-4 min-h-[420px]">
          <div className="flex items-center justify-between">
            <RecordingTabs active={activeTab} onChange={setActiveTab} />
            <span className="text-xs text-text/50">
              {t("recording.tabs.liveBadge")}
            </span>
          </div>
          <div className="flex-1 overflow-y-auto">{renderPanel()}</div>
        </div>

        {/* Right column (1/3) — cards */}
        <div className="flex flex-col gap-4">
          <AudioStatsCard />
          <RecordingInfoCard />
          <QuickNotesCard onAddNote={openAddNote} />
        </div>
      </div>

      <BottomControlsBar />
      <RecordingFooter />

      <AddNoteModal
        open={noteModalOpen}
        timestampSeconds={noteTimestamp}
        onClose={() => setNoteModalOpen(false)}
        onSubmit={handleSaveNote}
      />
    </div>
  );
};

export default RecordingView;
