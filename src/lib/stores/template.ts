import { writable } from "svelte/store";

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
