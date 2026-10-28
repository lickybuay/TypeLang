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
    toneSection: "Tono",
    toneHint: "Tono por defecto para nuevas traducciones. Podés cambiarlo al toque para un mensaje puntual presionando Tab en el popup.",
    toneProfessional: "Profesional",
    toneCasual: "Casual",
    toneSaved: "Tono guardado.",
    popupPlaceholder: "Escribe y presiona Enter…",
    popupTranslating: "Traduciendo…",
    popupHint: "Enter traducir · ⌘Enter guardar · Esc cancelar · Tab tono",
    snippetsSection: "Snippets",
    snippetsHint:
      "Frases guardadas para pegar directo con el segundo atajo, sin volver a traducir. Se crean solo: presioná ⌘Enter en el popup en vez de Enter para guardar esa traducción como snippet.",
    snippetShortcutLabel: "Atajo para abrir snippets",
    snippetLabelPlaceholder: "Etiqueta",
    snippetTextPlaceholder: "Texto a pegar",
    addSnippet: "+ Agregar snippet",
    snippetsEmptyList: "No hay snippets todavía — presioná ⌘Enter al traducir para guardar uno.",
    snippetSaved: "Snippet guardado.",
    snippetUpdated: "Snippet actualizado.",
    snippetDeleted: "Snippet eliminado.",
    snippetsSearchPlaceholder: "Buscar snippet…",
    snippetsEmptyState: "No hay snippets todavía",
    snippetsNoMatches: "Sin resultados",
    snippetsPickerHint: "↑↓ mover · Enter pegar · Esc cancelar",
    autostartSection: "Servicios locales",
    autostartLabel: "Levantar servicios locales al abrir TypeLang",
    autostartCommandLabel: "Comando",
    autostartHint:
      "Se ejecuta en un shell de login, en segundo plano. Si los servicios ya están corriendo, TypeLang no levanta otra copia y no los detiene al salir. Los logs van a ~/Library/Logs/typelang-services.log.",
    autostartSaved: "Servicios locales guardados.",
    popupTranscribing: "Transcribiendo…",
    micStart: "Dictar",
    micStop: "Detener y transcribir",
    dictationSection: "Dictado",
    dictationUrlLabel: "URL del servidor de dictado",
    dictationHint:
      "Servidor local de voz a texto compatible con OpenAI (ej. whisper.cpp). Click en el micrófono del popup para dictar en el idioma de origen; el texto se agrega al cuadro y lo podés editar antes de traducir.",
    vocabularyLabel: "Vocabulario",
    vocabularyPlaceholder: "Welcomy\nTypeLang",
    vocabularyHint:
      "Nombres y términos que deben escribirse exactos, uno por línea o separados por coma. Se usan al transcribir y al traducir.",
    vocabularySaved: "Vocabulario guardado.",
    dictationRemoteWarning: "Tu voz se enviará a {host}",
    dictationShortcutLabel: "Atajo para dictar sin ventana",
    dictationOutputLabel: "Salida del dictado",
    dictationOutputTranslate: "Traducir a {target}",
    dictationOutputKeep: "Mantener {source} (sin traducir)",
    dictationOutputTranslateHint: "Traduce lo que dijiste al idioma de destino.",
    dictationOutputKeepHint: "Pega lo que dijiste en {source}, sin traducirlo.",
    dictationDirectLabel: "Cuando no se traduce",
    dictationDirectImprove: "Mejorar (limpiar el texto)",
    dictationDirectOriginal: "Original (transcripción exacta)",
    dictationDirectImproveHint:
      "Mantiene tu idioma y tus palabras: solo corrige puntuación y errores de transcripción y quita muletillas.",
    dictationDirectOriginalHint: "Pega la transcripción tal cual, sin pasar por el modelo.",
    dictationOutputSaved: "Salida de dictado guardada.",
    dictationHotkeyHint:
      "Mantené presionado el atajo y hablá; al soltarlo se transcribe y se pega donde tengas el cursor, sin abrir el popup. Tocalo dos veces rápido para seguir grabando sin mantenerlo y presionalo de nuevo para terminar. Una píldora flotante muestra el tiempo, con ✓ para terminar y ✕ para descartar, y una etiqueta para alternar entre traducir y mantener tu idioma. El ícono de la barra de menú muestra el estado: ● grabando, … procesando, ⚠ error.",
    pillStarting: "Preparando el micrófono",
    pillLocked: "Grabación manos libres",
    pillAccept: "Terminar y pegar",
    pillCancel: "Descartar dictado",
    pillHint: "Mantené apretado, o tocá dos veces para seguir grabando",
    pillChipTranslate: "Traduciendo a {target} — clic para mantener {source}",
    pillChipKeepImprove: "Manteniendo {source} (mejorado) — clic para traducir",
    pillChipKeepOriginal: "Manteniendo {source} (original) — clic para traducir",
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
    toneSection: "Tone",
    toneHint: "Default tone for new translations. Override it for just one message from the popup by pressing Tab.",
    toneProfessional: "Professional",
    toneCasual: "Casual",
    toneSaved: "Tone saved.",
    popupPlaceholder: "Type and press Enter…",
    popupTranslating: "Translating…",
    popupHint: "Enter translate · ⌘Enter save · Esc cancel · Tab tone",
    snippetsSection: "Snippets",
    snippetsHint:
      "Saved phrases you can drop straight in with the second shortcut, without translating again. Created automatically: press ⌘Enter instead of Enter in the popup to save that translation as a snippet.",
    snippetShortcutLabel: "Shortcut to open snippets",
    snippetLabelPlaceholder: "Label",
    snippetTextPlaceholder: "Text to paste",
    addSnippet: "+ Add snippet",
    snippetsEmptyList: "No snippets yet — press ⌘Enter while translating to save one.",
    snippetSaved: "Snippet saved.",
    snippetUpdated: "Snippet updated.",
    snippetDeleted: "Snippet deleted.",
    snippetsSearchPlaceholder: "Search snippets…",
    snippetsEmptyState: "No snippets yet",
    snippetsNoMatches: "No matches",
    snippetsPickerHint: "↑↓ move · Enter paste · Esc cancel",
    autostartSection: "Local services",
    autostartLabel: "Start local services when TypeLang opens",
    autostartCommandLabel: "Command",
    autostartHint:
      "Runs in a login shell, in the background. If the services are already running, TypeLang won't start another copy and won't stop them on quit. Logs go to ~/Library/Logs/typelang-services.log.",
    autostartSaved: "Local services saved.",
    popupTranscribing: "Transcribing…",
    micStart: "Dictate",
    micStop: "Stop and transcribe",
    dictationSection: "Dictation",
    dictationUrlLabel: "Dictation server URL",
    dictationHint:
      "Local OpenAI-compatible speech-to-text server (e.g. whisper.cpp). Click the popup's mic to dictate in the source language; the text is added to the box and you can edit it before translating.",
    vocabularyLabel: "Vocabulary",
    vocabularyPlaceholder: "Welcomy\nTypeLang",
    vocabularyHint:
      "Names and terms that must be spelled exactly, one per line or comma-separated. Used when transcribing and when translating.",
    vocabularySaved: "Vocabulary saved.",
    dictationRemoteWarning: "Your voice will be sent to {host}",
    dictationShortcutLabel: "Shortcut to dictate without a window",
    dictationOutputLabel: "Dictation output",
    dictationOutputTranslate: "Translate to {target}",
    dictationOutputKeep: "Keep {source} (no translation)",
    dictationOutputTranslateHint: "Translates what you said into the target language.",
    dictationOutputKeepHint: "Pastes what you said in {source}, without translating it.",
    dictationDirectLabel: "When not translating",
    dictationDirectImprove: "Improve (clean up)",
    dictationDirectOriginal: "Original (exact transcript)",
    dictationDirectImproveHint:
      "Keeps your language and your words: only fixes punctuation and transcription mistakes and drops filler words.",
    dictationDirectOriginalHint: "Pastes the transcript as is, without going through the model.",
    dictationOutputSaved: "Dictation output saved.",
    dictationHotkeyHint:
      "Hold the shortcut and speak; on release it is transcribed and pasted at your cursor, without opening the popup. Tap it twice quickly to keep recording without holding it, and press it again to finish. A floating pill shows the time, with ✓ to finish and ✕ to discard, and a chip to switch between translating and keeping your language. The menu bar icon shows the state: ● recording, … working, ⚠ error.",
    pillStarting: "Getting the microphone ready",
    pillLocked: "Hands-free recording",
    pillAccept: "Finish and paste",
    pillCancel: "Discard dictation",
    pillHint: "Hold, or tap twice to keep recording",
    pillChipTranslate: "Translating to {target} — click to keep {source}",
    pillChipKeepImprove: "Keeping {source} (improved) — click to translate",
    pillChipKeepOriginal: "Keeping {source} (original) — click to translate",
  },
} as const;

export type DictKey = keyof (typeof dict)["en"];

export function t(lang: Lang, key: DictKey): string {
  return dict[lang]?.[key] ?? dict.en[key];
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

// Short codes for the pill's output chip, keyed by the names above.
const LANGUAGE_CODES: Record<string, string> = {
  Spanish: "ES",
  English: "EN",
  Portuguese: "PT",
  French: "FR",
  German: "DE",
  Italian: "IT",
};

export function languageCode(name: string): string {
  return LANGUAGE_CODES[name] ?? name.slice(0, 2).toUpperCase();
}

// What the hold-to-talk dictation pastes; mirrors `parse_dictation_output` in
// settings_store.rs. Anything unknown is "translate", as in the backend.
export type DictationOutput = "translate" | "improve" | "original";

export function parseDictationOutput(value: unknown): DictationOutput {
  return value === "improve" || value === "original" ? value : "translate";
}

// What "keep the language" pastes; chosen in Settings only. Mirrors
// `parse_direct_output`.
export type DirectOutput = "improve" | "original";

export function parseDirectOutput(value: unknown): DirectOutput {
  return value === "original" ? "original" : "improve";
}
