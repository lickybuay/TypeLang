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
// Mirrors audio::MAX_RECORDING_SECS in Rust, which caps the capture buffer.
const MAX_RECORDING_SECONDS = 120;

type RecState = "idle" | "recording" | "transcribing";

function formatElapsed(seconds: number) {
  const mm = String(Math.floor(seconds / 60)).padStart(2, "0");
  const ss = String(seconds % 60).padStart(2, "0");
  return `${mm}:${ss}`;
}

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
  const [recState, setRecState] = useState<RecState>("idle");
  const [elapsed, setElapsed] = useState(0);
  // Sticks once any text in this popup session came from the mic, even if
  // the user edits it afterwards — it only selects extra prompt rules.
  const fromVoiceRef = useRef(false);
  // Bumped on every reset so a transcription that finishes after the popup
  // was closed and reopened doesn't write into the new session.
  const sessionRef = useRef(0);
  const startingRef = useRef(false);
  const [appendTick, setAppendTick] = useState(0);
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
    // The window is only ever reset when it (re)appears, so anything still
    // recording or in flight belongs to the previous session.
    invoke("cancel_recording").catch(() => {});
    setRecState("idle");
    setElapsed(0);
    fromVoiceRef.current = false;
    sessionRef.current += 1;
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
      invoke("cancel_recording").catch(() => {});
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    resizeToContent();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [text, error, loading, recState]);

  // After a transcript lands: caret at the end, focus back in the textarea.
  useEffect(() => {
    const input = inputRef.current;
    if (!input || appendTick === 0) return;
    input.focus();
    input.setSelectionRange(input.value.length, input.value.length);
  }, [appendTick]);

  useEffect(() => {
    if (recState !== "recording") return;
    const startedAt = Date.now();
    const timer = window.setInterval(() => {
      const seconds = Math.floor((Date.now() - startedAt) / 1000);
      setElapsed(Math.min(seconds, MAX_RECORDING_SECONDS));
      if (seconds >= MAX_RECORDING_SECONDS) {
        window.clearInterval(timer);
        stopRecording();
      }
    }, 250);
    return () => window.clearInterval(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [recState]);

  // The Rust side cancels the capture when the window loses focus; this just
  // brings the button back to idle so it doesn't show a stale recording.
  useEffect(() => {
    const handleBlur = () => {
      if (recState !== "recording") return;
      invoke("cancel_recording").catch(() => {});
      setRecState("idle");
      setElapsed(0);
    };
    window.addEventListener("blur", handleBlur);
    return () => window.removeEventListener("blur", handleBlur);
  }, [recState]);

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

  const startRecording = async () => {
    if (startingRef.current) return;
    startingRef.current = true;
    setError(null);
    try {
      await invoke("start_recording");
      setElapsed(0);
      setRecState("recording");
    } catch (e) {
      setError(String(e));
    } finally {
      startingRef.current = false;
    }
  };

  const stopRecording = async () => {
    const session = sessionRef.current;
    setRecState("transcribing");
    try {
      const transcript = (await invoke<string>("stop_and_transcribe")).trim();
      if (session !== sessionRef.current) return;
      // Empty = recording too short or no speech: nothing to append.
      if (transcript) {
        fromVoiceRef.current = true;
        setText((prev) => (prev.trim() ? `${prev.trimEnd()} ${transcript}` : transcript));
        setAppendTick((n) => n + 1);
      }
    } catch (e) {
      if (session === sessionRef.current) setError(String(e));
    } finally {
      if (session === sessionRef.current) {
        setRecState("idle");
        setElapsed(0);
        inputRef.current?.focus();
      }
    }
  };

  const cancelRecording = () => {
    invoke("cancel_recording").catch(() => {});
    setRecState("idle");
    setElapsed(0);
    inputRef.current?.focus();
  };

  const toggleRecording = () => {
    if (recState === "idle") startRecording();
    else if (recState === "recording") stopRecording();
  };

  const submit = async (pin: boolean) => {
    if (!text.trim() || loading || recState !== "idle") return;
    setLoading(true);
    setError(null);
    try {
      // On success the Rust side hides the popup itself after pasting —
      // `pin` (Cmd+Enter instead of plain Enter) also saves the result as
      // a snippet, auto-labeled from the translated text.
      await invoke("translate_and_paste", {
        text,
        tone,
        pin,
        fromVoice: fromVoiceRef.current,
      });
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
          submit(e.metaKey || e.ctrlKey);
        } else if (e.key === "Escape") {
          e.preventDefault();
          if (recState === "recording") {
            // Cancels the dictation only — the popup stays open.
            cancelRecording();
          } else if (toneMenuOpen) {
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
              className={`popup-mic-button${recState === "recording" ? " recording" : ""}${
                recState === "transcribing" ? " transcribing" : ""
              }`}
              onClick={toggleRecording}
              disabled={loading || recState === "transcribing"}
              aria-label={t(lang, recState === "recording" ? "micStop" : "micStart")}
              title={t(lang, recState === "recording" ? "micStop" : "micStart")}
            >
              {recState === "transcribing" ? (
                <span className="popup-mic-spinner" aria-hidden="true" />
              ) : (
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                  <rect x="9" y="2" width="6" height="12" rx="3" />
                  <path d="M5 11a7 7 0 0 0 14 0" />
                  <path d="M12 18v4" />
                </svg>
              )}
              {recState === "recording" && (
                <span className="popup-mic-time">{formatElapsed(elapsed)}</span>
              )}
            </button>
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
        {recState === "transcribing" && (
          <div className="popup-status">{t(lang, "popupTranscribing")}</div>
        )}
        {error && <div className="popup-error">{error}</div>}
        {!loading && !error && recState !== "transcribing" && (
          <div className="popup-hint">{t(lang, "popupHint")}</div>
        )}
      </div>
    </div>
  );
}

export default Popup;
