import React, { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Plus, Check } from "lucide-react";
import { useTemplateStore } from "../../stores/templateStore";
import type { MeetingTemplate } from "@/bindings";

interface TemplatePickerProps {
  onSelect: (template: MeetingTemplate | null) => void;
  selectedTemplateId?: string | null;
  showNoneOption?: boolean;
}

/**
 * TemplatePicker - Component for selecting a meeting template
 *
 * Features:
 * - Displays templates in a grid layout
 * - Shows template icon, name, and audio source
 * - Supports "None" option to start without template
 * - Highlights selected template
 * - Auto-fetches templates on mount
 */
export const TemplatePicker: React.FC<TemplatePickerProps> = ({
  onSelect,
  selectedTemplateId = null,
  showNoneOption = true,
}) => {
  const { t } = useTranslation();
  const { templates, isLoading, error, fetchTemplates } = useTemplateStore();

  // Fetch templates on mount
  useEffect(() => {
    fetchTemplates();
  }, [fetchTemplates]);

  // Audio source display labels
  const getAudioSourceLabel = (audioSource: string): string => {
    switch (audioSource) {
      case "microphone_only":
        return t("meeting.template.audioSource.microphoneOnly", "Microphone");
      case "system_only":
        return t("meeting.template.audioSource.systemOnly", "System Audio");
      case "mixed":
        return t("meeting.template.audioSource.mixed", "Both");
      default:
        return audioSource;
    }
  };

  // Handle template selection
  const handleSelect = (template: MeetingTemplate | null) => {
    onSelect(template);
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-8">
        <span className="inline-flex h-5 w-5 rounded-full border-2 border-gray-400 border-t-transparent animate-spin"></span>
        <span className="ml-2 text-sm text-mid-gray">
          {t("meeting.template.loading", "Loading templates...")}
        </span>
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-4 bg-red-500/10 border border-red-500/30 rounded-lg">
        <p className="text-sm text-red-400">{error}</p>
      </div>
    );
  }

  return (
    <div className="space-y-3">
      {/* Section Title */}
      <h3 className="text-sm font-medium text-primary">
        {t("meeting.template.selectTemplate", "Select a template")}
      </h3>

      {/* Template Grid */}
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
        {/* None Option */}
        {showNoneOption && (
          <button
            onClick={() => handleSelect(null)}
            className={`
              group relative p-4 rounded-lg border-2 transition-all text-left
              ${
                selectedTemplateId === null
                  ? "border-blue-500 bg-blue-500/10"
                  : "border-gray-700 hover:border-gray-600 bg-gray-800/30"
              }
            `}
            aria-pressed={selectedTemplateId === null}
          >
            {/* Selection Indicator */}
            {selectedTemplateId === null && (
              <div className="absolute top-2 right-2">
                <Check size={16} className="text-blue-500" />
              </div>
            )}

            {/* Icon */}
            <div className="flex items-center gap-3 mb-2">
              <span className="text-2xl">📝</span>
              <span className="font-medium text-primary">
                {t("meeting.template.none", "None")}
              </span>
            </div>

            {/* Description */}
            <p className="text-xs text-mid-gray">
              {t(
                "meeting.template.noneDescription",
                "Start without a template",
              )}
            </p>
          </button>
        )}

        {/* Template Cards */}
        {templates.map((template) => (
          <button
            key={template.id}
            onClick={() => handleSelect(template)}
            className={`
              group relative p-4 rounded-lg border-2 transition-all text-left
              ${
                selectedTemplateId === template.id
                  ? "border-blue-500 bg-blue-500/10"
                  : "border-gray-700 hover:border-gray-600 bg-gray-800/30"
              }
            `}
            aria-pressed={selectedTemplateId === template.id}
          >
            {/* Selection Indicator */}
            {selectedTemplateId === template.id && (
              <div className="absolute top-2 right-2">
                <Check size={16} className="text-blue-500" />
              </div>
            )}

            {/* Icon and Name */}
            <div className="flex items-center gap-3 mb-2">
              <span className="text-2xl">{template.icon}</span>
              <span className="font-medium text-primary">{template.name}</span>
            </div>

            {/* Title Template Preview */}
            <p className="text-xs text-mid-gray mb-1 truncate">
              {template.title_template}
            </p>

            {/* Audio Source Badge */}
            <div className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs bg-gray-700/50 text-mid-gray">
              <span>{getAudioSourceLabel(template.audio_source)}</span>
            </div>
          </button>
        ))}
      </div>

      {/* Empty State */}
      {templates.length === 0 && (
        <div className="text-center py-8 text-sm text-mid-gray">
          <Plus size={24} className="mx-auto mb-2 opacity-50" />
          <p>
            {t(
              "meeting.template.noTemplates",
              "No templates yet. Create one in Settings.",
            )}
          </p>
        </div>
      )}
    </div>
  );
};

export default TemplatePicker;
