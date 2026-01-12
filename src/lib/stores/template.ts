import { writable } from "svelte/store";

/**
 * Annotation schema identifiers for template-coupled annotations.
 * Each schema defines which annotation types and options are available.
 */
export type AnnotationSchemaId = "tei-p5" | "menota";

export interface Template {
  id: string;
  name: string;
  description: string;
  header: string;
  footer: string;
  wordWrap: boolean;
  autoLineNumbers: boolean;
  multiLevel: boolean;
  wrapPages: boolean;
  validationSchemaId?: string;
  /**
   * Which annotation schema to use for this template.
   * - "tei-p5": Generic TEI P5 annotations
   * - "menota": MENOTA-specific annotations with proper attributes
   * Defaults to "menota" for multiLevel templates, "tei-p5" otherwise.
   */
  annotationSchemaId?: AnnotationSchemaId;
}

function createTemplateStore() {
  const { subscribe, set, update } = writable<{
    templates: Template[];
    active: Template | null;
  }>({
    templates: [],
    active: null,
  });

  return {
    subscribe,
    setTemplates: (templates: Template[]) =>
      update((state) => ({ ...state, templates })),
    setActive: (template: Template) =>
      update((state) => ({ ...state, active: template })),
    updateTemplate: (template: Template) =>
      update((state) => {
        const templates = state.templates.map((t) =>
          t.id === template.id ? template : t,
        );
        const active =
          state.active?.id === template.id ? template : state.active;
        return { ...state, templates, active };
      }),
    removeTemplate: (id: string) =>
      update((state) => {
        const templates = state.templates.filter((t) => t.id !== id);
        const active = state.active?.id === id ? null : state.active;
        return { ...state, templates, active };
      }),
    reset: () => set({ templates: [], active: null }),
  };
}

export const templateStore = createTemplateStore();
