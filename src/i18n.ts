export type Lang = "es" | "en";

const dict = {
  es: {
    appTitle: "TypeLang",
    uiLanguageSection: "Idioma del App",
    translationLangsSection: "Idiomas de traducción",
    from: "DE",
    to: "A",
    shortcutSection: "Configurar atajo de teclado",
    shortcutLabel: "Atajo",
    shortcutHint: "Haz clic y presiona la nueva combinación",
    shortcutRecording: "Presiona una combinación…",
    providerSection: "Proveedor de traducción",
    apiKeyLabel: "API key",
    statusConfigured: "Configurada",
    statusNotConfigured: "No configurada",
    save: "Guardar",
    saveConfig: "Guardar configuración",
    delete: "Eliminar",
    lmstudioUrlLabel: "URL del servidor",
    lmstudioOptional: " (opcional en LM Studio)",
    savedLocally: "API key guardada localmente",
    keySaved: "API key guardada.",
    keyDeleted: "API key eliminada.",
    urlSaved: "URL guardada.",
    shortcutSaved: "Atajo guardado.",
    settingsReadError: "Error al leer configuración",
    popupPlaceholder: "Escribe y presiona Enter…",
    popupTranslating: "Traduciendo…",
  },
  en: {
    appTitle: "TypeLang",
    uiLanguageSection: "App Language",
    translationLangsSection: "Translation languages",
    from: "FROM",
    to: "TO",
    shortcutSection: "Configure keyboard shortcut",
    shortcutLabel: "Shortcut",
    shortcutHint: "Click and press the new combination",
    shortcutRecording: "Press a key combination…",
    providerSection: "Translation provider",
    apiKeyLabel: "API key",
    statusConfigured: "Configured",
    statusNotConfigured: "Not configured",
    save: "Save",
    saveConfig: "Save configuration",
    delete: "Delete",
    lmstudioUrlLabel: "Server URL",
    lmstudioOptional: " (optional on LM Studio)",
    savedLocally: "API key saved locally",
    keySaved: "API key saved.",
    keyDeleted: "API key deleted.",
    urlSaved: "URL saved.",
    shortcutSaved: "Shortcut saved.",
    settingsReadError: "Error loading settings",
    popupPlaceholder: "Type and press Enter…",
    popupTranslating: "Translating…",
  },
} as const;

export type DictKey = keyof (typeof dict)["es"];

export function t(lang: Lang, key: DictKey): string {
  return dict[lang]?.[key] ?? dict.es[key];
}

export const LANGUAGE_OPTIONS = [
  { code: "es", label: "Español" },
  { code: "en", label: "English" },
] as const;

// Curated list, not exhaustive — the translation prompt is language-agnostic
// so more can be added later without touching the backend.
export const TRANSLATION_LANGUAGES = [
  "Spanish",
  "English",
  "Portuguese",
  "French",
  "German",
  "Italian",
] as const;
