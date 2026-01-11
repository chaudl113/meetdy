import React, { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { PromptTemplate, TemplateCategory } from "@/constants/promptTemplates";

interface TemplateDropdownProps {
  templates: PromptTemplate[];
  onSelect: (template: PromptTemplate) => void;
  onClose: () => void;
  currentlySelected?: string;
}

interface TemplateItemProps {
  template: PromptTemplate;
  isSelected: boolean;
  isFocused: boolean;
  onSelect: () => void;
}

const TemplateItem: React.FC<TemplateItemProps> = ({
  template,
  isSelected,
  isFocused,
  onSelect,
}) => {
  const { t } = useTranslation();
  const buttonRef = useRef<HTMLButtonElement>(null);

  // Scroll into view when focused
  useEffect(() => {
    if (isFocused && buttonRef.current) {
      buttonRef.current.scrollIntoView({ block: "nearest", behavior: "smooth" });
    }
  }, [isFocused]);

  return (
    <button
      ref={buttonRef}
      onClick={onSelect}
      className={`w-full text-left px-3 py-2 min-h-[44px] rounded-md transition-colors hover:bg-primary/10 ${
        isSelected ? "bg-primary/20 border-l-4 border-primary" : "border-l-4 border-transparent"
      } ${isFocused ? "bg-primary/20 ring-2 ring-inset ring-primary" : ""}`}
      role="option"
      id={template.id}
      aria-selected={isSelected}
      tabIndex={-1}
    >
      <div className="flex items-start gap-2">
        <span className="text-lg flex-shrink-0">{template.icon}</span>
        <div className="flex-1 min-w-0">
          <div className="text-sm font-medium text-text">
            {t(template.nameKey)}
          </div>
          <div className="text-xs text-mid-gray mt-0.5 line-clamp-2">
            {t(template.descriptionKey)}
          </div>
        </div>
        {isSelected && (
          // eslint-disable-next-line i18next/no-literal-string
          <span className="text-primary flex-shrink-0" aria-hidden="true">
            ✓
          </span>
        )}
      </div>
    </button>
  );
};

export const TemplateDropdown: React.FC<TemplateDropdownProps> = ({
  templates,
  onSelect,
  onClose,
  currentlySelected,
}) => {
  const { t } = useTranslation();
  const dropdownRef = useRef<HTMLDivElement>(null);
  const [focusedIndex, setFocusedIndex] = React.useState<number>(0);

  // Click outside to close
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (
        dropdownRef.current &&
        !dropdownRef.current.contains(e.target as Node)
      ) {
        onClose();
      }
    };

    const handleKeyDown = (e: KeyboardEvent) => {
      // Only handle keyboard events when dropdown is focused/active
      if (!dropdownRef.current?.contains(document.activeElement)) {
        return;
      }

      if (e.key === "Escape") {
        onClose();
      } else if (e.key === "ArrowDown") {
        e.preventDefault();
        if (templates.length === 0) return;
        setFocusedIndex((prev) => (prev + 1) % templates.length);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        if (templates.length === 0) return;
        setFocusedIndex((prev) => (prev - 1 + templates.length) % templates.length);
      } else if (e.key === "Enter" && focusedIndex >= 0 && focusedIndex < templates.length) {
        e.preventDefault();
        onSelect(templates[focusedIndex]);
      }
    };

    document.addEventListener("mousedown", handleClickOutside);
    document.addEventListener("keydown", handleKeyDown);

    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [onClose, templates, focusedIndex, onSelect]);

  // Group templates by category
  const groupedTemplates = templates.reduce(
    (acc, template) => {
      if (!acc[template.category]) {
        acc[template.category] = [];
      }
      acc[template.category].push(template);
      return acc;
    },
    {} as Record<TemplateCategory, PromptTemplate[]>,
  );

  const categoryLabels: Record<TemplateCategory, string> = {
    [TemplateCategory.Meeting]: t(
      "settings.postProcessing.prompts.categories.meeting",
    ),
    [TemplateCategory.Language]: t(
      "settings.postProcessing.prompts.categories.language",
    ),
    [TemplateCategory.Writing]: t(
      "settings.postProcessing.prompts.categories.writing",
    ),
  };

  return (
    <div
      ref={dropdownRef}
      className="absolute z-50 mt-2 w-full max-h-96 overflow-y-auto bg-background border border-mid-gray/20 rounded-lg shadow-lg animate-in fade-in slide-in-from-top-2 duration-200"
      role="listbox"
      aria-label={t("settings.postProcessing.prompts.templateList")}
      aria-activedescendant={templates[focusedIndex]?.id}
    >
      {Object.entries(groupedTemplates).map(([category, categoryTemplates]) => (
        <div key={category} className="p-2">
          <div
            className="px-3 py-2 text-xs font-semibold text-mid-gray uppercase"
            role="group"
            aria-label={categoryLabels[category as TemplateCategory]}
          >
            {categoryLabels[category as TemplateCategory]}
          </div>
          <div className="space-y-1">
            {categoryTemplates.map((template) => {
              const globalIndex = templates.findIndex(t => t.id === template.id);
              return (
                <TemplateItem
                  key={template.id}
                  template={template}
                  isSelected={template.id === currentlySelected}
                  isFocused={globalIndex === focusedIndex}
                  onSelect={() => onSelect(template)}
                />
              );
            })}
          </div>
        </div>
      ))}
    </div>
  );
};
