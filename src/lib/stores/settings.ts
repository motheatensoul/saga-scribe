import { writable, get } from "svelte/store";
import {
  loadSettings as loadSettingsFromBackend,
  saveSettings as saveSettingsToBackend,
  getSystemTheme,
  setWindowTheme,
  type Settings as BackendSettings,
} from "../tauri";

export interface Settings {
  fontSize: number;
  theme: "light" | "dark" | "system";
  autoPreview: boolean;
  previewDelay: number;
  activeTemplateId: string | null;
}

const defaultSettings: Settings = {
  fontSize: 14,
  theme: "system",
  autoPreview: true,
  previewDelay: 300,
  activeTemplateId: null,
};

let saveTimeout: ReturnType<typeof setTimeout> | null = null;

// Get effective theme considering system preference
async function getEffectiveTheme(
  theme: "light" | "dark" | "system",
): Promise<"light" | "dark"> {
  if (theme === "system") {
    try {
      // Use Tauri command to get real system theme (works on Linux/GNOME)
      const systemTheme = await getSystemTheme();
      return systemTheme === "dark" ? "dark" : "light";
    } catch (e) {
      console.warn(
        "Failed to get system theme, falling back to media query:",
        e,
      );
      // Fallback to media query
      return window.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light";
    }
  }
  return theme;
}

// Apply theme to HTML element
async function applyTheme(theme: "light" | "dark" | "system") {
  const effectiveTheme = await getEffectiveTheme(theme);
  const html = document.documentElement;

  // DaisyUI theme names
  const themeMap = {
    light: "caramellatte",
    dark: "coffee",
  };

  html.setAttribute("data-theme", themeMap[effectiveTheme]);

  // Cache in localStorage for instant application on page load
  try {
    localStorage.setItem("saga-scribe-theme", theme);
  } catch (e) {
    console.warn("Failed to cache theme in localStorage:", e);
  }

  // Update window decorations (titlebar theme on Linux/GNOME)
  try {
    await setWindowTheme(theme);
  } catch (e) {
    console.warn("Failed to set window theme:", e);
  }
}

function createSettingsStore() {
  const { subscribe, set, update } = writable<Settings>(defaultSettings);

  const debouncedSave = () => {
    if (saveTimeout) {
      clearTimeout(saveTimeout);
    }
    saveTimeout = setTimeout(async () => {
      const current = get({ subscribe });
      try {
        // Convert 'system' to 'light' for backend storage (backwards compatibility)
        const backendSettings: BackendSettings = {
          ...current,
          theme: current.theme === "system" ? "light" : current.theme,
        };
        await saveSettingsToBackend(backendSettings);
      } catch (e) {
        console.error("Failed to save settings:", e);
      }
    }, 500);
  };

  return {
    subscribe,
    load: async () => {
      try {
        const loaded = await loadSettingsFromBackend();
        const settings: Settings = {
          fontSize: loaded.fontSize,
          theme: (loaded.theme === "dark"
            ? "dark"
            : loaded.theme === "system"
              ? "system"
              : "light") as "light" | "dark" | "system",
          autoPreview: loaded.autoPreview,
          previewDelay: loaded.previewDelay,
          activeTemplateId: loaded.activeTemplateId,
        };
        set(settings);
        await applyTheme(settings.theme);
      } catch (e) {
        console.error("Failed to load settings, using defaults:", e);
        set(defaultSettings);
        await applyTheme(defaultSettings.theme);
      }
    },
    update: (partial: Partial<Settings>) => {
      update((state) => {
        const newState = { ...state, ...partial };
        if (partial.theme !== undefined) {
          // Apply theme asynchronously
          applyTheme(newState.theme).catch((e) =>
            console.error("Failed to apply theme:", e),
          );
        }
        return newState;
      });
      debouncedSave();
    },
    reset: () => {
      set(defaultSettings);
      applyTheme(defaultSettings.theme).catch((e) =>
        console.error("Failed to apply theme:", e),
      );
      debouncedSave();
    },
  };
}

export const settings = createSettingsStore();

// Listen for system theme changes when using 'system' theme
if (typeof window !== "undefined") {
  const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
  mediaQuery.addEventListener("change", () => {
    const currentSettings = get(settings);
    if (currentSettings.theme === "system") {
      applyTheme("system").catch((e) =>
        console.error("Failed to apply theme on system change:", e),
      );
    }
  });
}
