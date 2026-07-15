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
    updateSettings: "Actualizar configuración",
    delete: "Eliminar",
    lmstudioUrlLabel: "URL del servidor",
    localApiKeyOptional: " (opcional en servidores locales)",
    localModelLabel: "Modelo",
    localModelHint:
      "Solo se puede omitir si tenés un único modelo cargado. Si cargás varios a la vez en el mismo servidor (ej. uno chico para TypeLang y otro para programar), este campo es lo que rutea cada pedido al modelo correcto — debe coincidir exacto con el nombre que muestra tu servidor (ej. \"gemma-4-e4b-mlx\" en LM Studio, \"llama3.2\" en Ollama).",
    savedLocally: "Cifrado local seguro",
    keySaved: "API key guardada.",
    keyDeleted: "API key eliminada.",
    urlSaved: "URL guardada.",
    modelSaved: "Modelo guardado.",
    shortcutSaved: "Atajo guardado.",
    settingsUpdated: "Configuración actualizada.",
    settingsReadError: "Error al leer configuración",
    popupPlaceholder: "Escribe y presiona Enter…",
    popupTranslating: "Traduciendo…",
    popupHint: "Enter traducir · Esc cancelar",
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
    updateSettings: "Update settings",
    delete: "Delete",
    lmstudioUrlLabel: "Server URL",
    localApiKeyOptional: " (optional on local servers)",
    localModelLabel: "Model",
    localModelHint:
      "Only safe to leave blank if you have a single model loaded. If you load several at once on the same server (e.g. a small one for TypeLang alongside another for coding), this is what routes each request to the right one — must match exactly what your server shows (e.g. \"gemma-4-e4b-mlx\" on LM Studio, \"llama3.2\" on Ollama).",
    savedLocally: "Secure local encryption",
    keySaved: "API key saved.",
    keyDeleted: "API key deleted.",
    urlSaved: "URL saved.",
    modelSaved: "Model saved.",
    shortcutSaved: "Shortcut saved.",
    settingsUpdated: "Settings updated.",
    settingsReadError: "Error loading settings",
    popupPlaceholder: "Type and press Enter…",
    popupTranslating: "Translating…",
    popupHint: "Enter to translate · Esc to cancel",
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
