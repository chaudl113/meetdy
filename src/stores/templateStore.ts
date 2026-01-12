import { create } from "zustand";
import { subscribeWithSelector } from "zustand/middleware";
import type { MeetingTemplate } from "@/bindings";
import { commands } from "@/bindings";

interface TemplateStore {
  // State
  templates: MeetingTemplate[];
  selectedTemplate: MeetingTemplate | null;
  isLoading: boolean;
  error: string | null;

  // Actions
  fetchTemplates: () => Promise<void>;
  createTemplate: (
    name: string,
    icon: string,
    titleTemplate: string,
    audioSource: string,
    promptId: string | null,
    summaryPromptTemplate?: string | null,
  ) => Promise<MeetingTemplate | null>;
  updateTemplate: (
    id: string,
    name?: string,
    icon?: string,
    titleTemplate?: string,
    audioSource?: string,
    promptId?: string | null,
    summaryPromptTemplate?: string | null,
  ) => Promise<MeetingTemplate | null>;
  deleteTemplate: (id: string) => Promise<boolean>;
  selectTemplate: (template: MeetingTemplate | null) => void;
  clearError: () => void;

  // Internal setters
  setTemplates: (templates: MeetingTemplate[]) => void;
  setSelectedTemplate: (template: MeetingTemplate | null) => void;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
}

export const useTemplateStore = create<TemplateStore>()(
  subscribeWithSelector((set, get) => ({
    // Initial state
    templates: [],
    selectedTemplate: null,
    isLoading: false,
    error: null,

    // Internal setters
    setTemplates: (templates) => set({ templates }),
    setSelectedTemplate: (selectedTemplate) => set({ selectedTemplate }),
    setLoading: (isLoading) => set({ isLoading }),
    setError: (error) => set({ error }),

    // Clear error
    clearError: () => set({ error: null }),

    // Select a template for quick start
    selectTemplate: (template) => {
      set({ selectedTemplate: template });
    },

    // Fetch all templates from backend
    fetchTemplates: async () => {
      const { setTemplates, setError, setLoading } = get();

      setLoading(true);
      setError(null);

      try {
        const result = await commands.listMeetingTemplates();
        if (result.status === "ok") {
          setTemplates(result.data);
        } else {
          setError(result.error);
        }
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Failed to fetch templates";
        setError(errorMessage);
      } finally {
        setLoading(false);
      }
    },

    // Create a new template
    createTemplate: async (
      name,
      icon,
      titleTemplate,
      audioSource,
      promptId,
      summaryPromptTemplate,
    ) => {
      const { setError, setLoading, fetchTemplates } = get();

      setLoading(true);
      setError(null);

      try {
        const result = await commands.createMeetingTemplate(
          name,
          icon,
          titleTemplate,
          audioSource,
          promptId,
          summaryPromptTemplate ?? null,
        );

        if (result.status === "ok") {
          const newTemplate = result.data as MeetingTemplate;
          // Refresh template list to get updated data
          await fetchTemplates();
          return newTemplate;
        } else {
          setError(result.error);
          return null;
        }
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Failed to create template";
        setError(errorMessage);
        return null;
      } finally {
        setLoading(false);
      }
    },

    // Update an existing template
    updateTemplate: async (
      id,
      name,
      icon,
      titleTemplate,
      audioSource,
      promptId,
      summaryPromptTemplate,
    ) => {
      const { setError, setLoading, fetchTemplates } = get();

      setLoading(true);
      setError(null);

      try {
        const result = await commands.updateMeetingTemplate(
          id,
          name ?? null,
          icon ?? null,
          titleTemplate ?? null,
          audioSource ?? null,
          promptId ?? null,
          summaryPromptTemplate ?? null,
        );

        if (result.status === "ok") {
          const updatedTemplate = result.data as MeetingTemplate;
          // Refresh template list to get updated data
          await fetchTemplates();
          return updatedTemplate;
        } else {
          setError(result.error);
          return null;
        }
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Failed to update template";
        setError(errorMessage);
        return null;
      } finally {
        setLoading(false);
      }
    },

    // Delete a template
    deleteTemplate: async (id) => {
      const { setError, setLoading, fetchTemplates, selectedTemplate } = get();

      setLoading(true);
      setError(null);

      try {
        const result = await commands.deleteMeetingTemplate(id);

        if (result.status === "ok") {
          // Clear selection if the deleted template was selected
          if (selectedTemplate?.id === id) {
            set({ selectedTemplate: null });
          }
          // Refresh template list
          await fetchTemplates();
          return true;
        } else {
          setError(result.error);
          return false;
        }
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Failed to delete template";
        setError(errorMessage);
        return false;
      } finally {
        setLoading(false);
      }
    },
  })),
);
