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
  { id: "lmstudio", label: "LM Studio (local)" },
];

type SettingsView = {
  provider: string;
  lmstudio_base_url: string;
  ui_language: string;
  source_lang: string;
  target_lang: string;
  shortcut: string;
  has_api_key: boolean;
};

function SettingsPage() {
  const [lang, setLang] = useState<Lang>("es");
  const [provider, setProvider] = useState("anthropic");
  const [apiKey, setApiKey] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [sourceLang, setSourceLang] = useState("Spanish");
  const [targetLang, setTargetLang] = useState("English");
  const [shortcut, setShortcut] = useState("Alt+Shift+T");
  const [hasKey, setHasKey] = useState<boolean | null>(null);
  const [status, setStatus] = useState<string | null>(null);

  const refresh = async () => {
    try {
      const s = await invoke<SettingsView>("get_settings");
      setProvider(s.provider);
      setBaseUrl(s.lmstudio_base_url);
      setLang(s.ui_language === "en" ? "en" : "es");
      setSourceLang(s.source_lang);
      setTargetLang(s.target_lang);
      setShortcut(s.shortcut);
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

  const saveKey = async () => {
    if (!apiKey.trim()) return;
    try {
      await invoke("save_api_key", { provider, key: apiKey });
      setApiKey("");
      setStatus(t(lang, "keySaved"));
      await refresh();
    } catch (e) {
      setStatus(`Error: ${e}`);
    }
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

  const isLmStudio = provider === "lmstudio";

  return (
    <div className="settings">
      <header className="settings-header">
        <h1>{t(lang, "appTitle")}</h1>
      </header>

      <main className="settings-body">
        <section className="card">
          <h2>{t(lang, "uiLanguageSection")}</h2>
          <select
            className="pill"
            value={lang}
            onChange={(e) => changeUiLanguage(e.target.value as Lang)}
          >
            {LANGUAGE_OPTIONS.map((o) => (
              <option key={o.code} value={o.code}>
                {o.label}
              </option>
            ))}
          </select>
        </section>

        <section className="card">
          <h2>{t(lang, "translationLangsSection")}</h2>
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

        <section className="card">
          <h2>{t(lang, "shortcutSection")}</h2>
          <label className="field-label">{t(lang, "shortcutLabel")}</label>
          <ShortcutRecorder lang={lang} value={shortcut} onSave={saveShortcut} />
          <p className="hint">{t(lang, "shortcutHint")}</p>
        </section>

        <section className="card">
          <h2>{t(lang, "providerSection")}</h2>
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

          {isLmStudio && (
            <>
              <label className="field-label">{t(lang, "lmstudioUrlLabel")}</label>
              <input
                className="pill"
                type="text"
                placeholder="http://localhost:1234/v1"
                value={baseUrl}
                onChange={(e) => setBaseUrl(e.target.value)}
                onBlur={saveBaseUrl}
              />
            </>
          )}

          <div className="field-label-row">
            <label className="field-label">
              {t(lang, "apiKeyLabel")}
              {isLmStudio ? t(lang, "lmstudioOptional") : ""}
            </label>
            <span className={`status-badge ${hasKey ? "ok" : "warn"}`}>
              {hasKey ? t(lang, "statusConfigured") : t(lang, "statusNotConfigured")}
            </span>
          </div>
          <input
            className="pill"
            type="password"
            placeholder={isLmStudio ? "sk-… (opcional)" : "sk-..."}
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
          />
          <div className="settings-actions">
            <button
              className="pill pill-button pill-button-secondary"
              onClick={clearKey}
              disabled={!hasKey}
            >
              {t(lang, "delete")}
            </button>
          </div>
        </section>
      </main>

      <footer className="settings-footer">
        <button className="cta-button" onClick={saveKey} disabled={!apiKey.trim()}>
          {t(lang, "saveConfig")}
        </button>
        <p className="footer-caption">🔒 {t(lang, "savedLocally")}</p>
        {status && <p className="settings-message">{status}</p>}
      </footer>
    </div>
  );
}

export default SettingsPage;
