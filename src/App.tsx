import { useEffect, useState, useCallback } from "react";
import { Toaster, toast } from "sonner";
import "./App.css";
import AccessibilityPermissions from "./components/AccessibilityPermissions";
import Footer from "./components/footer";
import Onboarding from "./components/onboarding";
import { Sidebar, SidebarSection, SECTIONS_CONFIG } from "./components/Sidebar";
import { useSettings } from "./hooks/useSettings";
import { commands } from "@/bindings";
import { useSettingsStore } from "@/stores/settingsStore";
import { useMeetingStore } from "@/stores/meetingStore";

const renderSettingsContent = (section: SidebarSection) => {
  const ActiveComponent =
    SECTIONS_CONFIG[section]?.component || SECTIONS_CONFIG.general.component;
  return <ActiveComponent />;
};

function App() {
  const [showOnboarding, setShowOnboarding] = useState<boolean | null>(null);
  const [currentSection, setCurrentSection] =
    useState<SidebarSection>("general");
  const { settings, updateSetting } = useSettings();

  // Mode switching stores
  const { currentMode, setCurrentMode, isDictationRecording } = useSettingsStore();
  const { sessionStatus, stopMeeting } = useMeetingStore();

  useEffect(() => {
    checkOnboardingStatus();
  }, []);

  // Handle keyboard shortcuts for debug mode toggle
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      // Check for Ctrl+Shift+D (Windows/Linux) or Cmd+Shift+D (macOS)
      const isDebugShortcut =
        event.shiftKey &&
        event.key.toLowerCase() === "d" &&
        (event.ctrlKey || event.metaKey);

      if (isDebugShortcut) {
        event.preventDefault();
        const currentDebugMode = settings?.debug_mode ?? false;
        updateSetting("debug_mode", !currentDebugMode);
      }
    };

    // Add event listener when component mounts
    document.addEventListener("keydown", handleKeyDown);

    // Cleanup event listener when component unmounts
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [settings?.debug_mode, updateSetting]);

  const checkOnboardingStatus = async () => {
    try {
      // Always check if they have any models available
      const result = await commands.hasAnyModelsAvailable();
      if (result.status === "ok") {
        setShowOnboarding(!result.data);
      } else {
        setShowOnboarding(true);
      }
    } catch (error) {
      console.error("Failed to check onboarding status:", error);
      setShowOnboarding(true);
    }
  };

  const handleModelSelected = () => {
    // Transition to main app - user has started a download
    setShowOnboarding(false);
  };

  /**
   * Handles section changes with mode mutual exclusivity.
   * When switching to meeting mode, stops any active dictation.
   * When switching from meeting mode, prompts confirmation if recording is active.
   */
  const handleSectionChange = useCallback(
    async (newSection: SidebarSection) => {
      const isEnteringMeeting = newSection === "meeting";
      const isLeavingMeeting = currentSection === "meeting" && newSection !== "meeting";
      const isMeetingRecording = sessionStatus === "recording";

      // Case 1: Switching TO meeting mode
      if (isEnteringMeeting) {
        // Check if dictation is currently recording
        const dictationActive = await isDictationRecording();
        if (dictationActive) {
          // Dictation recording is active - it will be stopped by the backend
          // when user starts a meeting. For now, just notify user.
          toast.info("Dictation will be stopped when you start a meeting.");
        }
        setCurrentMode("meeting");
        setCurrentSection(newSection);
        return;
      }

      // Case 2: Switching FROM meeting mode while recording
      if (isLeavingMeeting && isMeetingRecording) {
        // Show confirmation toast with action buttons
        toast("Stop meeting recording?", {
          description: "Switching sections will stop the current recording.",
          action: {
            label: "Stop & Switch",
            onClick: async () => {
              await stopMeeting();
              setCurrentMode("dictation");
              setCurrentSection(newSection);
            },
          },
          cancel: {
            label: "Cancel",
            onClick: () => {
              // Do nothing - stay on meeting section
            },
          },
          duration: 10000,
        });
        return;
      }

      // Case 3: Leaving meeting mode (not recording)
      if (isLeavingMeeting) {
        setCurrentMode("dictation");
      }

      // Default: just switch sections
      setCurrentSection(newSection);
    },
    [currentSection, sessionStatus, isDictationRecording, stopMeeting, setCurrentMode]
  );

  if (showOnboarding) {
    return <Onboarding onModelSelected={handleModelSelected} />;
  }

  return (
    <div className="h-screen flex flex-col">
      <Toaster />
      {/* Main content area that takes remaining space */}
      <div className="flex-1 flex overflow-hidden">
        <Sidebar
          activeSection={currentSection}
          onSectionChange={handleSectionChange}
        />
        {/* Scrollable content area */}
        <div className="flex-1 flex flex-col overflow-hidden">
          <div className="flex-1 overflow-y-auto">
            <div className="flex flex-col items-center p-4 gap-4">
              <AccessibilityPermissions />
              {renderSettingsContent(currentSection)}
            </div>
          </div>
        </div>
      </div>
      {/* Fixed footer at bottom */}
      <Footer />
    </div>
  );
}

export default App;
