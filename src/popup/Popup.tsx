import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { t } from "../i18n";
import type { Lang } from "../i18n";
import "./popup.css";

type SettingsView = {
  ui_language: string;
  source_lang: string;
  target_lang: string;
  tone: string;
};

const WINDOW_WIDTH = 480;
const MIN_WINDOW_HEIGHT = 206;
const MAX_WINDOW_HEIGHT = 436;
// Window padding (18px top+bottom) that isn't accounted for by the card's
// own scrollHeight — needs room for the card's drop-shadow to fully fade
// out instead of getting clipped by the window edge.
const WINDOW_CHROME = 36;

function Popup() {
  const [text, setText] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [lang, setLang] = useState<Lang>("en");
  const [sourceLang, setSourceLang] = useState("Spanish");
  const [targetLang, setTargetLang] = useState("English");
  // Starts as the app-wide default (from Settings) every time the popup
  // opens, but Tab flips it for just this message without touching that
  // default — see the translate_and_paste call in submit().
  const [tone, setTone] = useState<"professional" | "casual">("professional");
  const [toneMenuOpen, setToneMenuOpen] = useState(false);
  const toneWrapRef = useRef<HTMLDivElement>(null);
  // Bumped on every "popup-reset" so the card below remounts and replays
  // its CSS entrance animation — the window itself is reused (hidden, not
  // destroyed) across shortcut presses, so a plain mount-only animation
  // would only ever play once.
  const [revealKey, setRevealKey] = useState(0);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const cardRef = useRef<HTMLDivElement>(null);

  const resizeToContent = () => {
    const textarea = inputRef.current;
    const card = cardRef.current;
    if (!textarea || !card) return;

    textarea.style.height = "auto";
    textarea.style.height = `${textarea.scrollHeight}px`;

    const desired = card.scrollHeight + WINDOW_CHROME;
    const clamped = Math.min(MAX_WINDOW_HEIGHT, Math.max(MIN_WINDOW_HEIGHT, desired));
    getCurrentWindow()
      .setSize(new LogicalSize(WINDOW_WIDTH, clamped))
      .catch(() => {
        // Non-fatal — the textarea itself already scrolls past max-height.
      });
  };

  const fetchLangs = async () => {
    try {
      const s = await invoke<SettingsView>("get_settings");
      setLang(s.ui_language === "es" ? "es" : "en");
      setSourceLang(s.source_lang);
      setTargetLang(s.target_lang);
      setTone(s.tone === "casual" ? "casual" : "professional");
    } catch {
      // Non-fatal — popup still works with the last-known/default langs.
    }
  };

  const reset = () => {
    setText("");
    setError(null);
    setLoading(false);
    setToneMenuOpen(false);
    setRevealKey((k) => k + 1);
    inputRef.current?.focus();
    if (inputRef.current) inputRef.current.style.height = "";
    getCurrentWindow()
      .setSize(new LogicalSize(WINDOW_WIDTH, MIN_WINDOW_HEIGHT))
      .catch(() => {});
    fetchLangs();
  };

  useEffect(() => {
    inputRef.current?.focus();
    fetchLangs();
    resizeToContent();
    const unlisten = listen("popup-reset", reset);
    return () => {
      unlisten.then((f) => f());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    resizeToContent();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [text, error, loading]);

  useEffect(() => {
    if (!toneMenuOpen) return;
    const handleClickOutside = (e: MouseEvent) => {
      if (!toneWrapRef.current?.contains(e.target as Node)) {
        setToneMenuOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [toneMenuOpen]);

  const selectTone = (next: "professional" | "casual") => {
    // Same per-message override as Tab — just a second way to reach it.
    setTone(next);
    setToneMenuOpen(false);
    inputRef.current?.focus();
  };

  const submit = async () => {
    if (!text.trim() || loading) return;
    setLoading(true);
    setError(null);
    try {
      // On success the Rust side hides the popup itself after pasting.
      await invoke("translate_and_paste", { text, tone });
    } catch (e) {
      setError(String(e));
      setLoading(false);
    }
  };

  const cancel = () => {
    invoke("cancel_popup");
  };

  const openSettings = () => {
    invoke("open_settings_from_popup");
  };

  return (
    <div
      className="popup"
      onKeyDown={(e) => {
        if (e.key === "Enter" && !e.shiftKey) {
          e.preventDefault();
          submit();
        } else if (e.key === "Escape") {
          e.preventDefault();
          if (toneMenuOpen) {
            setToneMenuOpen(false);
          } else {
            cancel();
          }
        } else if (e.key === "Tab") {
          // Per-message override, not a settings change — resets to the
          // app-wide default the next time the popup opens (see reset()).
          e.preventDefault();
          setToneMenuOpen(false);
          setTone((prev) => (prev === "professional" ? "casual" : "professional"));
        }
      }}
    >
      <div className="popup-card" ref={cardRef} key={revealKey}>
        <div className="popup-langs" data-tauri-drag-region="">
          <span>
            {sourceLang} → {targetLang}
          </span>
          <div className="popup-header-right">
            <div className="tone-picker" ref={toneWrapRef}>
              <button
                type="button"
                className={`tone-badge tone-${tone}`}
                onClick={() => setToneMenuOpen((v) => !v)}
                aria-haspopup="listbox"
                aria-expanded={toneMenuOpen}
              >
                {t(lang, tone === "casual" ? "toneCasual" : "toneProfessional")}
                <svg width="8" height="8" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3">
                  <path d="M6 9l6 6 6-6" />
                </svg>
              </button>
              {toneMenuOpen && (
                <div className="tone-menu" role="listbox">
                  <button
                    type="button"
                    role="option"
                    aria-selected={tone === "professional"}
                    className={`tone-menu-option${tone === "professional" ? " active" : ""}`}
                    onClick={() => selectTone("professional")}
                  >
                    {t(lang, "toneProfessional")}
                  </button>
                  <button
                    type="button"
                    role="option"
                    aria-selected={tone === "casual"}
                    className={`tone-menu-option${tone === "casual" ? " active" : ""}`}
                    onClick={() => selectTone("casual")}
                  >
                    {t(lang, "toneCasual")}
                  </button>
                </div>
              )}
            </div>
            <button
              type="button"
              className="popup-settings-button"
              onClick={openSettings}
              aria-label="Settings"
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <circle cx="12" cy="12" r="3" />
                <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
              </svg>
            </button>
          </div>
        </div>
        <textarea
          ref={inputRef}
          className="pill popup-input"
          value={text}
          disabled={loading}
          onChange={(e) => setText(e.target.value)}
          placeholder={t(lang, "popupPlaceholder")}
          rows={3}
          autoFocus
        />
        {loading && <div className="popup-status">{t(lang, "popupTranslating")}</div>}
        {error && <div className="popup-error">{error}</div>}
        {!loading && !error && <div className="popup-hint">{t(lang, "popupHint")}</div>}
      </div>
    </div>
  );
}

export default Popup;
