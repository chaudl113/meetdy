import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { PromptTemplate, BUILTIN_TEMPLATES } from "@/constants/promptTemplates";
import { TemplateDropdown } from "./TemplateDropdown";
import { TemplatePreview } from "./TemplatePreview";
import { Button } from "@/components/ui/Button";
import { commands } from "@/bindings";
import { useSettings } from "@/hooks/useSettings";

interface TemplateSelectorProps {
  onTemplateApplied?: (template: PromptTemplate) => void;
}

export const TemplateSelector: React.FC<TemplateSelectorProps> = ({
  onTemplateApplied,
}) => {
  const { t } = useTranslation();
  const { refreshSettings } = useSettings();
  const [isDropdownOpen, setIsDropdownOpen] = useState(false);
  const [selectedTemplate, setSelectedTemplate] =
    useState<PromptTemplate | null>(null);
  const [previewMode, setPreviewMode] = useState<"preview" | "edit">("preview");
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSelectTemplate = (template: PromptTemplate) => {
    setSelectedTemplate(template);
    setPreviewMode("preview");
    setIsDropdownOpen(false);
    setError(null); // Clear any previous errors
  };

  const handleUseTemplate = () => {
    if (selectedTemplate && onTemplateApplied) {
      onTemplateApplied(selectedTemplate);
    }
    setSelectedTemplate(null);
    setError(null);
  };

  const handleSaveAsCustom = async (name: string, prompt: string) => {
    // Validation: Check empty name
    if (!name || name.trim().length === 0) {
      setError(t("settings.postProcessing.prompts.errors.emptyName"));
      return;
    }

    // Validation: Check empty prompt
    if (!prompt || prompt.trim().length === 0) {
      setError(t("settings.postProcessing.prompts.errors.emptyPrompt"));
      return;
    }

    // Validation: Check prompt contains ${output}
    if (!prompt.includes("${output}")) {
      setError(t("settings.postProcessing.prompts.errors.missingPlaceholder"));
      return;
    }

    // Validation: Check maximum length (1000 chars for name, 5000 for prompt)
    if (name.length > 1000) {
      setError(t("settings.postProcessing.prompts.errors.nameTooLong"));
      return;
    }

    if (prompt.length > 5000) {
      setError(t("settings.postProcessing.prompts.errors.promptTooLong"));
      return;
    }

    setIsSaving(true);
    setError(null);

    try {
      const result = await commands.addPostProcessPrompt(name.trim(), prompt.trim());
      if (result.status === "ok") {
        await refreshSettings();
        setSelectedTemplate(null);
        setPreviewMode("preview");
        // Success - could add toast notification here
      } else if (result.status === "error") {
        setError(result.error || t("settings.postProcessing.prompts.errors.saveFailed"));
      }
    } catch (error) {
      console.error("Failed to save custom prompt:", error);
      setError(
        error instanceof Error
          ? error.message
          : t("settings.postProcessing.prompts.errors.saveFailed")
      );
    } finally {
      setIsSaving(false);
    }
  };

  const handleCancel = () => {
    setSelectedTemplate(null);
    setPreviewMode("preview");
    setError(null);
  };

  const handleEditTemplate = () => {
    setPreviewMode("edit");
    setError(null);
  };

  return (
    <div className="space-y-3">
      {/* Template Selector Button */}
      <div className="relative">
        <div className="flex gap-2">
          <button
            onClick={() => setIsDropdownOpen(!isDropdownOpen)}
            className="flex-1 px-4 py-2 text-sm border border-mid-gray/20 rounded-lg hover:bg-mid-gray/5 focus:outline-none focus:ring-2 focus:ring-primary transition-colors flex items-center justify-between"
            aria-haspopup="listbox"
            aria-expanded={isDropdownOpen}
          >
            <span className="flex items-center gap-2">
              <span>{t("settings.postProcessing.prompts.useTemplate")}</span>
            </span>
            {/* eslint-disable-next-line i18next/no-literal-string */}
            <span className="text-mid-gray" aria-hidden="true">
              ▼
            </span>
          </button>
        </div>

        {/* Dropdown */}
        {isDropdownOpen && (
          <TemplateDropdown
            templates={BUILTIN_TEMPLATES}
            onSelect={handleSelectTemplate}
            onClose={() => setIsDropdownOpen(false)}
            currentlySelected={selectedTemplate?.id}
          />
        )}
      </div>

      {/* Template Preview */}
      {selectedTemplate && (
        <div className="relative">
          {/* Error message */}
          {error && (
            <div
              className="mb-3 px-4 py-3 rounded-lg bg-red-500/10 border border-red-500/20 text-red-600 text-sm flex items-start gap-2"
              role="alert"
            >
              {/* eslint-disable-next-line i18next/no-literal-string */}
              <span className="text-base flex-shrink-0" aria-hidden="true">
                ⚠️
              </span>
              <span className="flex-1">{error}</span>
            </div>
          )}

          <TemplatePreview
            template={selectedTemplate}
            mode={previewMode}
            onUseTemplate={handleUseTemplate}
            onSaveCustom={handleSaveAsCustom}
            onCancel={handleCancel}
            isSaving={isSaving}
          />
          {previewMode === "preview" && (
            <div className="mt-2">
              <button
                onClick={handleEditTemplate}
                className="text-sm text-primary hover:underline focus:outline-none focus:ring-2 focus:ring-primary focus:ring-offset-2 rounded"
              >
                {t("settings.postProcessing.prompts.editAndSave")}
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  );
};
