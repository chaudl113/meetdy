import React, { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { PromptTemplate } from "@/constants/promptTemplates";
import { Input } from "@/components/ui/Input";
import { Textarea } from "@/components/ui/Textarea";
import { Button } from "@/components/ui/Button";

interface TemplatePreviewProps {
  template: PromptTemplate;
  mode: "preview" | "edit";
  onUseTemplate: () => void;
  onSaveCustom: (name: string, prompt: string) => void;
  onCancel: () => void;
  isSaving?: boolean;
}

export const TemplatePreview: React.FC<TemplatePreviewProps> = ({
  template,
  mode,
  onUseTemplate,
  onSaveCustom,
  onCancel,
  isSaving = false,
}) => {
  const { t } = useTranslation();
  const [editedName, setEditedName] = useState("");
  const [editedPrompt, setEditedPrompt] = useState("");

  useEffect(() => {
    if (mode === "edit") {
      setEditedName(t(template.nameKey));
      setEditedPrompt(template.prompt);
    }
  }, [template, mode, t]);

  const handleSave = () => {
    if (editedName.trim() && editedPrompt.trim()) {
      onSaveCustom(editedName.trim(), editedPrompt.trim());
    }
  };

  const highlightPlaceholder = (text: string) => {
    return text.replace(
      /\$\{output\}/g,
      '<code class="px-1 py-0.5 bg-primary/10 text-primary rounded text-sm font-mono">${output}</code>',
    );
  };

  return (
    <div className="space-y-3 p-4 bg-mid-gray/5 rounded-lg border border-mid-gray/20">
      {/* Template source indicator */}
      <div className="flex items-center gap-2 text-sm text-mid-gray">
        <span>{template.icon}</span>
        <span>
          {t("settings.postProcessing.prompts.fromTemplate")}:{" "}
          <span className="font-medium text-text">{t(template.nameKey)}</span>
        </span>
      </div>

      {mode === "preview" ? (
        <>
          {/* Preview Mode: Read-only display */}
          <div className="space-y-3">
            <div>
              <label className="text-sm font-semibold block mb-2">
                {t("settings.postProcessing.prompts.promptLabel")}
              </label>
              <div className="px-3 py-2 bg-background rounded border border-mid-gray/20 text-sm">
                {t(template.nameKey)}
              </div>
            </div>

            <div>
              <label className="text-sm font-semibold block mb-2">
                {t("settings.postProcessing.prompts.promptInstructions")}
              </label>
              <div
                className="px-3 py-2 bg-background rounded border border-mid-gray/20 text-sm whitespace-pre-wrap"
                dangerouslySetInnerHTML={{
                  __html: highlightPlaceholder(template.prompt),
                }}
              />
            </div>

            <p className="text-xs text-mid-gray/70">
              💡 {t("settings.postProcessing.prompts.outputPlaceholderTip")}
            </p>
          </div>

          {/* Action buttons for preview */}
          <div className="flex gap-2 pt-2">
            <Button onClick={onUseTemplate} variant="primary" size="md">
              {t("settings.postProcessing.prompts.useTemplate")}
            </Button>
            <Button onClick={onCancel} variant="secondary" size="md">
              {t("common.cancel")}
            </Button>
          </div>
        </>
      ) : (
        <>
          {/* Edit Mode: Editable fields */}
          <div className="space-y-3">
            <div>
              <label className="text-sm font-semibold block mb-2">
                {t("settings.postProcessing.prompts.promptLabel")}
              </label>
              <Input
                type="text"
                value={editedName}
                onChange={(e) => setEditedName(e.target.value)}
                placeholder={t(
                  "settings.postProcessing.prompts.promptLabelPlaceholder",
                )}
                variant="compact"
              />
            </div>

            <div>
              <label className="text-sm font-semibold block mb-2">
                {t("settings.postProcessing.prompts.promptInstructions")}
              </label>
              <Textarea
                value={editedPrompt}
                onChange={(e) => setEditedPrompt(e.target.value)}
                placeholder={t(
                  "settings.postProcessing.prompts.promptInstructionsPlaceholder",
                )}
                rows={10}
              />
              <p className="text-xs text-mid-gray/70 mt-2">
                💡 {t("settings.postProcessing.prompts.outputPlaceholderTip")}
              </p>
            </div>
          </div>

          {/* Action buttons for edit */}
          <div className="flex gap-2 pt-2">
            <Button
              onClick={handleSave}
              variant="primary"
              size="md"
              disabled={!editedName.trim() || !editedPrompt.trim() || isSaving}
            >
              {isSaving
                ? t("common.saving")
                : t("settings.postProcessing.prompts.saveAsCustom")}
            </Button>
            <Button onClick={onCancel} variant="secondary" size="md" disabled={isSaving}>
              {t("common.cancel")}
            </Button>
          </div>
        </>
      )}
    </div>
  );
};
