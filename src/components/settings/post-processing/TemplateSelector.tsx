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

  const handleSelectTemplate = (template: PromptTemplate) => {
    setSelectedTemplate(template);
    setPreviewMode("preview");
    setIsDropdownOpen(false);
  };

  const handleUseTemplate = () => {
    if (selectedTemplate && onTemplateApplied) {
      onTemplateApplied(selectedTemplate);
    }
    setSelectedTemplate(null);
  };

  const handleSaveAsCustom = async (name: string, prompt: string) => {
    try {
      const result = await commands.addPostProcessPrompt(name, prompt);
      if (result.status === "ok") {
        await refreshSettings();
        setSelectedTemplate(null);
        // Show success toast (optional - can add toast later)
        console.log("Custom prompt saved successfully");
      }
    } catch (error) {
      console.error("Failed to save custom prompt:", error);
      // Show error toast (optional)
    }
  };

  const handleCancel = () => {
    setSelectedTemplate(null);
    setPreviewMode("preview");
  };

  const handleEditTemplate = () => {
    setPreviewMode("edit");
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
          <TemplatePreview
            template={selectedTemplate}
            mode={previewMode}
            onUseTemplate={handleUseTemplate}
            onSaveCustom={handleSaveAsCustom}
            onCancel={handleCancel}
          />
          {previewMode === "preview" && (
            <div className="mt-2">
              <button
                onClick={handleEditTemplate}
                className="text-sm text-primary hover:underline"
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
