import React from "react";
import { useMeetingStore } from "../../stores/meetingStore";
import { StartMeeting } from "./StartMeeting";
import { RecordingView } from "./RecordingView";
import { CompletedView } from "./CompletedView";

/**
 * MeetingMode - Root container for the meeting feature.
 *
 * Routes the UI based on `sessionStatus`:
 *  - idle                    -> StartMeeting form (configure & start)
 *  - recording               -> RecordingView (live recording UI)
 *  - processing/completed/
 *    failed/interrupted      -> Post-recording view (status + transcript)
 *
 * Event listeners are initialized at the App level (App.tsx) so they persist
 * across section switches. No need to init/cleanup here.
 */
export const MeetingMode: React.FC = () => {
  const sessionStatus = useMeetingStore((s) => s.sessionStatus);

  if (sessionStatus === "idle") {
    return <StartMeeting />;
  }

  if (sessionStatus === "recording") {
    return <RecordingView />;
  }

  // processing / completed / failed / interrupted
  return <CompletedView />;
};

export default MeetingMode;
