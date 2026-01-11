/**
 * Built-in prompt templates for Post-Processing feature
 * Each template provides a ready-to-use prompt for common use cases
 */

export enum TemplateCategory {
  Meeting = "meeting",
  Language = "language",
  Writing = "writing",
}

export interface PromptTemplate {
  id: string;
  /** i18n key for template name */
  nameKey: string;
  /** i18n key for template description */
  descriptionKey: string;
  /** Template category for grouping */
  category: TemplateCategory;
  /** The actual prompt text with ${output} placeholder */
  prompt: string;
  /** Icon identifier (emoji or icon name) */
  icon: string;
}

/**
 * Built-in prompt templates
 * Note: Template prompts use ${output} as placeholder for transcribed text
 */
export const BUILTIN_TEMPLATES: PromptTemplate[] = [
  // Meeting Templates
  {
    id: "meeting-summary",
    nameKey: "settings.postProcessing.prompts.templates.meetingSummary.name",
    descriptionKey:
      "settings.postProcessing.prompts.templates.meetingSummary.description",
    category: TemplateCategory.Meeting,
    icon: "✅",
    prompt: `Analyze the following meeting transcript and create a structured summary with these sections:

1. **Overview**: Brief summary (2-3 sentences)
2. **Key Discussion Points**: Main topics discussed
3. **Decisions Made**: Concrete decisions or conclusions
4. **Action Items**: Tasks identified (if any)
5. **Next Steps**: Future plans or follow-ups

Transcript:
\${output}`,
  },
  {
    id: "action-items",
    nameKey: "settings.postProcessing.prompts.templates.actionItems.name",
    descriptionKey:
      "settings.postProcessing.prompts.templates.actionItems.description",
    category: TemplateCategory.Meeting,
    icon: "📝",
    prompt: `Extract all action items, tasks, and to-dos from the following transcript. Format as a numbered list with:
- Task description
- Assigned person (if mentioned)
- Deadline (if mentioned)

If no action items found, respond: "No action items identified."

Transcript:
\${output}`,
  },
  {
    id: "key-points",
    nameKey: "settings.postProcessing.prompts.templates.keyPoints.name",
    descriptionKey:
      "settings.postProcessing.prompts.templates.keyPoints.description",
    category: TemplateCategory.Meeting,
    icon: "💡",
    prompt: `Extract the key points from the following transcript and present them as a concise bullet list. Focus on main ideas and important information only.

Transcript:
\${output}`,
  },

  // Language Templates
  {
    id: "translate-vi",
    nameKey: "settings.postProcessing.prompts.templates.translateVi.name",
    descriptionKey:
      "settings.postProcessing.prompts.templates.translateVi.description",
    category: TemplateCategory.Language,
    icon: "🌐",
    prompt: `Translate the following text to Vietnamese. Maintain the original tone and meaning. If technical terms are present, keep them in English with Vietnamese explanation in parentheses.

Text:
\${output}`,
  },

  // Writing Templates
  {
    id: "grammar-clarity",
    nameKey: "settings.postProcessing.prompts.templates.grammarClarity.name",
    descriptionKey:
      "settings.postProcessing.prompts.templates.grammarClarity.description",
    category: TemplateCategory.Writing,
    icon: "✏️",
    prompt: `Improve the grammar, spelling, and clarity of the following text while preserving the original meaning and tone. Fix any errors and make the text more readable.

Text:
\${output}`,
  },
  {
    id: "email-draft",
    nameKey: "settings.postProcessing.prompts.templates.emailDraft.name",
    descriptionKey:
      "settings.postProcessing.prompts.templates.emailDraft.description",
    category: TemplateCategory.Writing,
    icon: "📧",
    prompt: `Convert the following notes into a professional email format with:
- Appropriate greeting
- Clear subject line suggestion (in brackets)
- Well-structured body paragraphs
- Professional closing

Notes:
\${output}`,
  },
  {
    id: "technical-notes",
    nameKey: "settings.postProcessing.prompts.templates.technicalNotes.name",
    descriptionKey:
      "settings.postProcessing.prompts.templates.technicalNotes.description",
    category: TemplateCategory.Writing,
    icon: "🔧",
    prompt: `Format the following technical discussion into structured documentation with:
- Summary section
- Technical details with code/commands in markdown code blocks
- Key decisions or specifications
- References or links (if mentioned)

Use proper markdown formatting.

Content:
\${output}`,
  },
];

/**
 * Get a template by its ID
 */
export function getTemplateById(id: string): PromptTemplate | undefined {
  return BUILTIN_TEMPLATES.find((template) => template.id === id);
}

/**
 * Get templates filtered by category
 */
export function getTemplatesByCategory(
  category: TemplateCategory,
): PromptTemplate[] {
  return BUILTIN_TEMPLATES.filter((template) => template.category === category);
}

/**
 * Get all templates grouped by category
 */
export function getTemplatesGroupedByCategory(): Record<
  TemplateCategory,
  PromptTemplate[]
> {
  return BUILTIN_TEMPLATES.reduce(
    (acc, template) => {
      if (!acc[template.category]) {
        acc[template.category] = [];
      }
      acc[template.category].push(template);
      return acc;
    },
    {} as Record<TemplateCategory, PromptTemplate[]>,
  );
}
