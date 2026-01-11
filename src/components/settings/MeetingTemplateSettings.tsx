import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Plus, Pencil, Trash2, X, Check } from "lucide-react";
import { useTemplateStore } from "../../stores/templateStore";
import { useSettings } from "../../hooks/useSettings";
import { SettingsGroup } from "../ui/SettingsGroup";
import { SettingContainer } from "../ui/SettingContainer";
import { Button } from "../ui/Button";
import { Input } from "../ui/Input";
import { Dropdown } from "../ui/Dropdown";
import type { MeetingTemplate } from "@/bindings";

const AUDIO_SOURCE_OPTIONS = [
  { value: "microphone_only", label: "Microphone Only" },
  { value: "system_only", label: "System Audio Only" },
  { value: "mixed", label: "Both (Mixed)" },
];

const DEFAULT_ICONS = ["👥", "☕", "🎤", "📞", "💼", "🎯", "📝", "🗓️"];

export const MeetingTemplateSettings: React.FC = () => {
  const { t } = useTranslation();
  const {
    templates,
    isLoading,
    error,
    fetchTemplates,
    createTemplate,
    updateTemplate,
    deleteTemplate,
  } = useTemplateStore();

  const { getSetting } = useSettings();
  const prompts = getSetting("post_process_prompts") || [];

  // Local state for form
  const [isEditing, setIsEditing] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [formData, setFormData] = useState({
    name: "",
    icon: "👥",
    titleTemplate: "",
    audioSource: "microphone_only",
    promptId: null as string | null,
  });

  // Fetch templates on mount
  useEffect(() => {
    fetchTemplates();
  }, [fetchTemplates]);

  // Handle create new template
  const handleCreate = () => {
    setIsEditing(true);
    setEditingId(null);
    setFormData({
      name: "",
      icon: "👥",
      titleTemplate: "",
      audioSource: "microphone_only",
      promptId: null,
    });
  };

  // Handle edit existing template
  const handleEdit = (template: MeetingTemplate) => {
    setIsEditing(true);
    setEditingId(template.id);
    setFormData({
      name: template.name,
      icon: template.icon,
      titleTemplate: template.title_template,
      audioSource: template.audio_source,
      promptId: template.prompt_id,
    });
  };

  // Handle save (create or update)
  const handleSave = async () => {
    if (!formData.name.trim()) {
      return;
    }

    try {
      if (editingId) {
        // Update existing
        await updateTemplate(
          editingId,
          formData.name,
          formData.icon,
          formData.titleTemplate,
          formData.audioSource,
          formData.promptId,
        );
      } else {
        // Create new
        await createTemplate(
          formData.name,
          formData.icon,
          formData.titleTemplate,
          formData.audioSource,
          formData.promptId,
        );
      }

      // Reset form
      setIsEditing(false);
      setEditingId(null);
    } catch (err) {
      console.error("Failed to save template:", err);
    }
  };

  // Handle cancel
  const handleCancel = () => {
    setIsEditing(false);
    setEditingId(null);
  };

  // Handle delete
  const handleDelete = async (id: string) => {
    if (
      confirm(
        t(
          "settings.meeting.template.confirmDelete",
          "Are you sure you want to delete this template?",
        ),
      )
    ) {
      await deleteTemplate(id);
    }
  };

  // Prepare prompt options for dropdown
  const promptOptions = [
    { value: "", label: t("settings.meeting.template.noPrompt", "None") },
    ...prompts.map((prompt) => ({
      value: prompt.id,
      label: prompt.name,
    })),
  ];

  return (
    <SettingsGroup
      title={t("settings.meeting.template.title", "Meeting Templates")}
    >
      <div className="space-y-4 p-4">
        {/* Description */}
        <p className="text-sm text-mid-gray">
          {t(
            "settings.meeting.template.description",
            "Create templates for different meeting types to quickly start recordings with pre-configured settings.",
          )}
        </p>

        {/* Error Display */}
        {error && (
          <div className="p-3 bg-red-500/10 border border-red-500/30 rounded-lg">
            <p className="text-sm text-red-400">{error}</p>
          </div>
        )}

        {/* Loading State */}
        {isLoading && !isEditing && (
          <div className="flex items-center gap-2 text-sm text-mid-gray">
            <span className="inline-flex h-4 w-4 rounded-full border-2 border-gray-400 border-t-transparent animate-spin"></span>
            <span>{t("common.loading", "Loading...")}</span>
          </div>
        )}

        {/* Template List */}
        {!isEditing && (
          <div className="space-y-2">
            {templates.map((template) => {
              const isDefault = template.id.startsWith("template_");
              return (
                <div
                  key={template.id}
                  className="flex items-center gap-3 p-3 bg-gray-800/30 border border-gray-700 rounded-lg"
                >
                  {/* Icon */}
                  <span className="text-2xl">{template.icon}</span>

                  {/* Template Info */}
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="font-medium text-primary">
                        {template.name}
                      </span>
                      {isDefault && (
                        <span className="text-xs px-1.5 py-0.5 rounded bg-blue-500/20 text-blue-400">
                          {t("settings.meeting.template.default", "Default")}
                        </span>
                      )}
                    </div>
                    <p className="text-xs text-mid-gray truncate">
                      {template.title_template}
                    </p>
                    <div className="flex items-center gap-2 mt-1">
                      <span className="text-xs px-1.5 py-0.5 rounded bg-gray-700/50 text-mid-gray">
                        {AUDIO_SOURCE_OPTIONS.find(
                          (opt) => opt.value === template.audio_source,
                        )?.label || template.audio_source}
                      </span>
                      {template.prompt_id && (
                        <span className="text-xs px-1.5 py-0.5 rounded bg-gray-700/50 text-mid-gray">
                          {prompts.find((p) => p.id === template.prompt_id)
                            ?.name || t("common.prompt", "Prompt")}
                        </span>
                      )}
                    </div>
                  </div>

                  {/* Action Buttons */}
                  {!isDefault && (
                    <div className="flex items-center gap-1">
                      <button
                        onClick={() => handleEdit(template)}
                        className="p-2 text-mid-gray hover:text-primary transition-colors rounded hover:bg-gray-700/50"
                        aria-label={t("common.edit", "Edit")}
                      >
                        <Pencil size={16} />
                      </button>
                      <button
                        onClick={() => handleDelete(template.id)}
                        className="p-2 text-mid-gray hover:text-red-400 transition-colors rounded hover:bg-red-500/10"
                        aria-label={t("common.delete", "Delete")}
                      >
                        <Trash2 size={16} />
                      </button>
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}

        {/* Create/Edit Form */}
        {isEditing && (
          <div className="space-y-4 p-4 bg-gray-800/30 border border-gray-700 rounded-lg">
            <h4 className="font-medium text-primary">
              {editingId
                ? t("settings.meeting.template.edit", "Edit Template")
                : t("settings.meeting.template.create", "Create Template")}
            </h4>

            {/* Template Name */}
            <SettingContainer
              title={t("settings.meeting.template.name", "Name")}
              description={t(
                "settings.meeting.template.nameDescription",
                "A descriptive name for this template",
              )}
              descriptionMode="tooltip"
              layout="stacked"
              grouped={true}
            >
              <Input
                value={formData.name}
                onChange={(e) =>
                  setFormData({ ...formData, name: e.target.value })
                }
                placeholder={t(
                  "settings.meeting.template.namePlaceholder",
                  "e.g., Team Standup",
                )}
                maxLength={50}
                className="w-full"
              />
              <p className="text-xs text-mid-gray mt-1">
                {formData.name.length}/50 {t("common.characters", "characters")}
              </p>
            </SettingContainer>

            {/* Icon Selector */}
            <SettingContainer
              title={t("settings.meeting.template.icon", "Icon")}
              description={t(
                "settings.meeting.template.iconDescription",
                "Choose an emoji icon",
              )}
              descriptionMode="tooltip"
              layout="stacked"
              grouped={true}
            >
              <div className="flex gap-2 flex-wrap">
                {DEFAULT_ICONS.map((icon) => (
                  <button
                    key={icon}
                    onClick={() => setFormData({ ...formData, icon })}
                    className={`
                      text-2xl p-2 rounded transition-colors
                      ${
                        formData.icon === icon
                          ? "bg-blue-500/20 ring-2 ring-blue-500"
                          : "hover:bg-gray-700/50"
                      }
                    `}
                  >
                    {icon}
                  </button>
                ))}
                <Input
                  value={formData.icon}
                  onChange={(e) =>
                    setFormData({ ...formData, icon: e.target.value })
                  }
                  placeholder="Or type emoji"
                  className="w-20 text-center"
                  maxLength={2}
                />
              </div>
            </SettingContainer>

            {/* Title Template */}
            <SettingContainer
              title={t("settings.meeting.template.titleTemplate", "Title Template")}
              description={t(
                "settings.meeting.template.titleTemplateDescription",
                "Template for meeting title. Use {date} and {time} placeholders.",
              )}
              descriptionMode="tooltip"
              layout="stacked"
              grouped={true}
            >
              <Input
                value={formData.titleTemplate}
                onChange={(e) =>
                  setFormData({ ...formData, titleTemplate: e.target.value })
                }
                placeholder={t(
                  "settings.meeting.template.titleTemplatePlaceholder",
                  "e.g., Standup - {date}",
                )}
                className="w-full"
              />
            </SettingContainer>

            {/* Audio Source */}
            <SettingContainer
              title={t("settings.meeting.template.audioSource", "Audio Source")}
              description={t(
                "settings.meeting.template.audioSourceDescription",
                "Default audio source for this template",
              )}
              descriptionMode="tooltip"
              layout="horizontal"
              grouped={true}
            >
              <Dropdown
                options={AUDIO_SOURCE_OPTIONS}
                value={formData.audioSource}
                onChange={(value) =>
                  setFormData({ ...formData, audioSource: value })
                }
                className="min-w-[200px]"
              />
            </SettingContainer>

            {/* Prompt Selection */}
            <SettingContainer
              title={t("settings.meeting.template.prompt", "AI Prompt")}
              description={t(
                "settings.meeting.template.promptDescription",
                "Optional AI prompt for post-processing",
              )}
              descriptionMode="tooltip"
              layout="horizontal"
              grouped={true}
            >
              <Dropdown
                options={promptOptions}
                value={formData.promptId || ""}
                onChange={(value) =>
                  setFormData({ ...formData, promptId: value || null })
                }
                className="min-w-[200px]"
              />
            </SettingContainer>

            {/* Action Buttons */}
            <div className="flex items-center gap-2 pt-2">
              <Button
                variant="primary"
                size="sm"
                onClick={handleSave}
                disabled={!formData.name.trim() || isLoading}
                className="flex items-center gap-1.5"
              >
                <Check size={16} />
                {t("common.save", "Save")}
              </Button>
              <Button
                variant="secondary"
                size="sm"
                onClick={handleCancel}
                disabled={isLoading}
                className="flex items-center gap-1.5"
              >
                <X size={16} />
                {t("common.cancel", "Cancel")}
              </Button>
            </div>
          </div>
        )}

        {/* Create New Button */}
        {!isEditing && (
          <Button
            variant="secondary"
            size="sm"
            onClick={handleCreate}
            disabled={isLoading}
            className="flex items-center gap-2"
          >
            <Plus size={16} />
            {t("settings.meeting.template.createNew", "Create Template")}
          </Button>
        )}
      </div>
    </SettingsGroup>
  );
};

export default MeetingTemplateSettings;
