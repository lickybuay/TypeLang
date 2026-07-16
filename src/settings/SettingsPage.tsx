import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import ShortcutRecorder from "./ShortcutRecorder";
import { LANGUAGE_OPTIONS, TRANSLATION_LANGUAGES, t } from "../i18n";
import type { Lang } from "../i18n";
import "./settings.css";

const PROVIDERS = [
  { id: "anthropic", label: "Anthropic (Claude)" },
  { id: "openai", label: "OpenAI" },
  { id: "gemini", label: "Google Gemini" },
  { id: "lmstudio", label: "Local LLM" },
];

// Small inline icons for each card heading — same stroke style as the
// popup's gear icon, sized to sit inline with the uppercase section
// labels. aria-hidden since the label text already names the section.
const iconProps = {
  width: 13,
  height: 13,
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 2,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
  "aria-hidden": true,
};

function IconLanguages() {
  return (
    <svg {...iconProps}>
      <path d="M8 3 4 7l4 4" />
      <path d="M4 7h16" />
      <path d="M16 21l4-4-4-4" />
      <path d="M20 17H4" />
    </svg>
  );
}

function IconKeyboard() {
  return (
    <svg {...iconProps}>
      <rect x="2" y="6" width="20" height="12" rx="2" />
      <path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M6 14h12" />
    </svg>
  );
}

function IconTone() {
  return (
    <svg {...iconProps}>
      <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
    </svg>
  );
}

function IconServer() {
  return (
    <svg {...iconProps}>
      <rect x="3" y="4" width="18" height="7" rx="1.5" />
      <rect x="3" y="13" width="18" height="7" rx="1.5" />
      <path d="M7 8h.01M7 17h.01" />
    </svg>
  );
}

// Show/hide toggle for the API key field — the one field on this page that
// actually hides its value (type="password"), so a reveal affordance has
// real use, unlike Server URL/Model which are already plain text.
function IconEye({ open }: { open: boolean }) {
  const eyeProps = { width: 16, height: 16, viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", strokeWidth: 2, strokeLinecap: "round" as const, strokeLinejoin: "round" as const, "aria-hidden": true };
  return open ? (
    <svg {...eyeProps}>
      <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8Z" />
      <circle cx="12" cy="12" r="3" />
    </svg>
  ) : (
    <svg {...eyeProps}>
      <path d="M17.94 17.94A10.94 10.94 0 0 1 12 20c-7 0-11-8-11-8a20.3 20.3 0 0 1 4.22-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a20.4 20.4 0 0 1-2.16 3.19M14.12 14.12a3 3 0 1 1-4.24-4.24" />
      <path d="M1 1l22 22" />
    </svg>
  );
}

type SettingsView = {
  provider: string;
  lmstudio_base_url: string;
  local_model: string;
  ui_language: string;
  source_lang: string;
  target_lang: string;
  shortcut: string;
  tone: string;
  has_api_key: boolean;
};

function SettingsPage() {
  const [lang, setLang] = useState<Lang>("en");
  const [provider, setProvider] = useState("anthropic");
  const [apiKey, setApiKey] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [localModel, setLocalModel] = useState("");
  const [sourceLang, setSourceLang] = useState("Spanish");
  const [targetLang, setTargetLang] = useState("English");
  const [shortcut, setShortcut] = useState("Alt+Shift+T");
  const [tone, setTone] = useState("professional");
  const [hasKey, setHasKey] = useState<boolean | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [showKey, setShowKey] = useState(false);

  const refresh = async () => {
    try {
      const s = await invoke<SettingsView>("get_settings");
      setProvider(s.provider);
      setBaseUrl(s.lmstudio_base_url);
      setLocalModel(s.local_model);
      setLang(s.ui_language === "es" ? "es" : "en");
      setSourceLang(s.source_lang);
      setTargetLang(s.target_lang);
      setShortcut(s.shortcut);
      setTone(s.tone === "casual" ? "casual" : "professional");
      setHasKey(s.has_api_key);
    } catch (e) {
      setStatus(`${t(lang, "settingsReadError")}: ${e}`);
    }
  };

  useEffect(() => {
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const changeUiLanguage = async (next: Lang) => {
    setLang(next);
    try {
      await invoke("save_ui_language", { language: next });
    } catch (e) {
      setStatus(`Error: ${e}`);
    }
  };

  const changeTranslationLangs = async (next: { from?: string; to?: string }) => {
    const nextSource = next.from ?? sourceLang;
    const nextTarget = next.to ?? targetLang;
    setSourceLang(nextSource);
    setTargetLang(nextTarget);
    try {
      await invoke("save_translation_langs", {
        sourceLang: nextSource,
        targetLang: nextTarget,
      });
    } catch (e) {
      setStatus(`Error: ${e}`);
    }
  };

  const changeTone = async (next: string) => {
    setTone(next);
    try {
      await invoke("save_tone", { tone: next });
      setStatus(t(lang, "toneSaved"));
    } catch (e) {
      setStatus(`Error: ${e}`);
    }
  };

  const saveShortcut = async (next: string) => {
    setShortcut(next);
    try {
      await invoke("save_shortcut", { shortcut: next });
      setStatus(t(lang, "shortcutSaved"));
    } catch (e) {
      setStatus(`Error: ${e}`);
    }
  };

  const changeProvider = async (next: string) => {
    setProvider(next);
    setApiKey("");
    setStatus(null);
    try {
      await invoke("save_provider", { provider: next });
    } catch (e) {
      setStatus(`Error: ${e}`);
    }
    await refresh();
  };

  // Every other field on this page already saves itself on blur — this is
  // the one exception, since typing an API key then losing focus (e.g. to
  // check something) shouldn't silently persist a half-typed secret. The
  // button used to be scoped (and named) to just that, but it reads as a
  // general "save my changes" action, so it always confirms now instead of
  // sitting disabled whenever the key field happens to be empty.
  const updateSettings = async () => {
    if (apiKey.trim()) {
      try {
        await invoke("save_api_key", { provider, key: apiKey });
        setApiKey("");
        setStatus(t(lang, "keySaved"));
        await refresh();
      } catch (e) {
        setStatus(`Error: ${e}`);
      }
      return;
    }
    setStatus(t(lang, "settingsUpdated"));
  };

  const clearKey = async () => {
    try {
      await invoke("clear_api_key", { provider });
      setStatus(t(lang, "keyDeleted"));
    } catch (e) {
      setStatus(`Error: ${e}`);
    }
    await refresh();
  };

  const saveBaseUrl = async () => {
    try {
      await invoke("save_lmstudio_base_url", { url: baseUrl });
      setStatus(t(lang, "urlSaved"));
    } catch (e) {
      setStatus(`Error: ${e}`);
    }
  };

  const saveLocalModel = async () => {
    try {
      await invoke("save_local_model", { model: localModel });
      setStatus(t(lang, "modelSaved"));
    } catch (e) {
      setStatus(`Error: ${e}`);
    }
  };

  const isLocalLlm = provider === "lmstudio";

  return (
    <div className="settings">
      <header className="settings-header">
        <div className="settings-header-top">
          <h1>{t(lang, "appTitle")}</h1>
          {/* App language is a one-time preference, not part of what the
              app does day to day — a compact corner control instead of
              its own card, same treatment macOS System Settings gives
              locale. */}
          <select
            className="pill header-lang-toggle"
            value={lang}
            onChange={(e) => changeUiLanguage(e.target.value as Lang)}
            aria-label={t(lang, "uiLanguageSection")}
          >
            {LANGUAGE_OPTIONS.map((o) => (
              <option key={o.code} value={o.code}>
                {o.code.toUpperCase()}
              </option>
            ))}
          </select>
        </div>
        {/* Live readout of the actual translation direction, not just a
            static wordmark — the header earns its color by saying what
            the app is about to do, the same "X → Y" language the popup
            uses for the same purpose. */}
        <p className="settings-tagline">
          {sourceLang} <span aria-hidden="true">→</span> {targetLang}
        </p>
      </header>

      <main className="settings-body">
        <div className="card">
        <section className="card-section">
          <h2>
            <IconLanguages />
            {t(lang, "translationLangsSection")}
          </h2>
          <div className="lang-pair">
            <div className="lang-pair-field">
              <span className="lang-pair-label">{t(lang, "from")}</span>
              <select
                className="pill"
                value={sourceLang}
                onChange={(e) => changeTranslationLangs({ from: e.target.value })}
              >
                {TRANSLATION_LANGUAGES.map((l) => (
                  <option key={l} value={l}>
                    {l}
                  </option>
                ))}
              </select>
            </div>
            <div className="lang-pair-field">
              <span className="lang-pair-label">{t(lang, "to")}</span>
              <select
                className="pill"
                value={targetLang}
                onChange={(e) => changeTranslationLangs({ to: e.target.value })}
              >
                {TRANSLATION_LANGUAGES.map((l) => (
                  <option key={l} value={l}>
                    {l}
                  </option>
                ))}
              </select>
            </div>
          </div>
        </section>

        <section className="card-section">
          <h2>
            <IconTone />
            {t(lang, "toneSection")}
          </h2>
          <div className="tone-toggle" role="group" aria-label={t(lang, "toneSection")}>
            <button
              type="button"
              className={`tone-toggle-button${tone === "professional" ? " active" : ""}`}
              onClick={() => changeTone("professional")}
            >
              {t(lang, "toneProfessional")}
            </button>
            <button
              type="button"
              className={`tone-toggle-button${tone === "casual" ? " active" : ""}`}
              onClick={() => changeTone("casual")}
            >
              {t(lang, "toneCasual")}
            </button>
          </div>
          <p className="hint">{t(lang, "toneHint")}</p>
        </section>

        <section className="card-section">
          <h2>
            <IconKeyboard />
            {t(lang, "shortcutSection")}
          </h2>
          <label className="field-label">{t(lang, "shortcutLabel")}</label>
          <ShortcutRecorder lang={lang} value={shortcut} onSave={saveShortcut} />
          <p className="hint">{t(lang, "shortcutHint")}</p>
        </section>

        <section className="card-section">
          <h2>
            <IconServer />
            {t(lang, "providerSection")}
          </h2>
          <select
            className="pill"
            value={provider}
            onChange={(e) => changeProvider(e.target.value)}
          >
            {PROVIDERS.map((p) => (
              <option key={p.id} value={p.id}>
                {p.label}
              </option>
            ))}
          </select>

          {isLocalLlm && (
            <>
              <label className="field-label">{t(lang, "lmstudioUrlLabel")}</label>
              <input
                className="pill pill-mono"
                type="text"
                placeholder="http://localhost:1234/v1"
                value={baseUrl}
                onChange={(e) => setBaseUrl(e.target.value)}
                onBlur={saveBaseUrl}
              />

              <label className="field-label">{t(lang, "localModelLabel")}</label>
              <input
                className="pill pill-mono"
                type="text"
                placeholder="e.g. gemma-4-e4b-mlx, llama3.2"
                value={localModel}
                onChange={(e) => setLocalModel(e.target.value)}
                onBlur={saveLocalModel}
              />
              <p className="hint">{t(lang, "localModelHint")}</p>
            </>
          )}

          <div className="field-label-row">
            <label className="field-label">
              {t(lang, "apiKeyLabel")}
              {isLocalLlm ? t(lang, "localApiKeyOptional") : ""}
            </label>
            <span className={`status-badge ${hasKey ? "ok" : "warn"}`}>
              {hasKey ? t(lang, "statusConfigured") : t(lang, "statusNotConfigured")}
            </span>
          </div>
          <div className="field-with-action">
            <input
              className="pill"
              type={showKey ? "text" : "password"}
              placeholder={isLocalLlm ? "sk-… (opcional)" : "sk-..."}
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
            />
            <button
              type="button"
              className="field-action-button"
              onClick={() => setShowKey((v) => !v)}
              aria-label={showKey ? "Hide API key" : "Show API key"}
            >
              <IconEye open={showKey} />
            </button>
          </div>
          {hasKey && (
            <div className="settings-actions">
              <button
                className="pill pill-button pill-button-secondary"
                onClick={clearKey}
              >
                {t(lang, "delete")}
              </button>
            </div>
          )}
        </section>
        </div>
      </main>

      <footer className="settings-footer">
        <button className="cta-button" onClick={updateSettings}>
          {t(lang, "updateSettings")}
          <span aria-hidden="true">→</span>
        </button>
        <p className="footer-caption">🔒 {t(lang, "savedLocally")}</p>
        {status && <p className="settings-message">{status}</p>}
      </footer>
    </div>
  );
}

export default SettingsPage;
