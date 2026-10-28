import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { languageCode, parseDictationOutput, t } from "../i18n";
import type { DictationOutput, Lang } from "../i18n";
import "./pill.css";

type PillState = "starting" | "recording" | "locked" | "processing" | "hint" | "error" | "notice";

// Mirrors PillPayload in dictation_hotkey.rs.
type PillPayload = {
  state: PillState;
  startedAtMs?: number;
  message?: string;
};

type SettingsView = {
  ui_language: string;
  source_lang: string;
  target_lang: string;
  dictation_output: string;
};

function formatElapsed(seconds: number) {
  const mm = String(Math.floor(seconds / 60)).padStart(2, "0");
  const ss = String(seconds % 60).padStart(2, "0");
  return `${mm}:${ss}`;
}

function Pill() {
  const [payload, setPayload] = useState<PillPayload | null>(null);
  const [lang, setLang] = useState<Lang>("en");
  const [output, setOutput] = useState<DictationOutput>("translate");
  const [sourceLang, setSourceLang] = useState("Spanish");
  const [targetLang, setTargetLang] = useState("English");
  const [now, setNow] = useState(() => Date.now());
  // A pill-state event that lands before the on-mount query answers is newer
  // than that answer.
  const gotEventRef = useRef(false);

  const loadSettings = () => {
    invoke<SettingsView>("get_settings")
      .then((s) => {
        setLang(s.ui_language === "es" ? "es" : "en");
        setOutput(parseDictationOutput(s.dictation_output));
        setSourceLang(s.source_lang);
        setTargetLang(s.target_lang);
      })
      .catch(() => {
        // Non-fatal — the pill keeps the last-known/default values.
      });
  };

  useEffect(() => {
    let alive = true;
    const unlisten = listen<PillPayload>("pill-state", (event) => {
      gotEventRef.current = true;
      setPayload(event.payload);
      // The window is reused across sessions: pick up a language or output
      // change made in Settings since the last one.
      if (event.payload.state === "starting") loadSettings();
    });
    // The output changed from Settings (or from this chip): no pill re-sync,
    // just the chip.
    const unlistenOutput = listen<{ output: string }>("dictation-output-changed", (event) => {
      setOutput(parseDictationOutput(event.payload.output));
    });
    invoke<PillPayload | null>("dictation_pill_state")
      .then((current) => {
        if (alive && current && !gotEventRef.current) setPayload(current);
      })
      .catch(() => {});
    loadSettings();
    return () => {
      alive = false;
      unlisten.then((f) => f());
      unlistenOutput.then((f) => f());
    };
  }, []);

  const startedAtMs = payload?.startedAtMs;
  const timed = startedAtMs !== undefined && (payload?.state === "recording" || payload?.state === "locked");
  useEffect(() => {
    if (!timed) return;
    setNow(Date.now());
    const timer = window.setInterval(() => setNow(Date.now()), 250);
    return () => window.clearInterval(timer);
  }, [timed, startedAtMs]);

  if (!payload) return null;

  const { state } = payload;
  const hasButtons = state !== "hint" && state !== "error" && state !== "notice";
  // Two states only: translating, or keeping the spoken language. Whether
  // keeping means improved or original is a Settings choice.
  const src = languageCode(sourceLang);
  const translating = output === "translate";
  const chipLabel = translating ? `${src}→${languageCode(targetLang)}` : src;
  const chipTitle = t(
    lang,
    translating ? "pillChipTranslate" : output === "improve" ? "pillChipKeepImprove" : "pillChipKeepOriginal",
  )
    .replace("{source}", sourceLang)
    .replace("{target}", targetLang);
  const elapsed = startedAtMs === undefined ? 0 : Math.max(0, Math.floor((now - startedAtMs) / 1000));

  // Rendered only in the starting, recording and locked looks below; the
  // processing, hint, error and notice looks have no chip.
  const chip = (
    <button
      type="button"
      className="rec-pill-chip"
      onMouseDown={(e) => e.preventDefault()}
      onClick={() =>
        invoke<string>("dictation_pill_toggle_output")
          .then((next) => setOutput(parseDictationOutput(next)))
          .catch(() => {})
      }
      aria-label={chipTitle}
      title={chipTitle}
    >
      {chipLabel}
    </button>
  );

  return (
    <div className={`rec-pill rec-pill-${state}`}>
      {hasButtons && (
        <button
          type="button"
          className="rec-pill-button"
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => invoke("dictation_pill_cancel").catch(() => {})}
          aria-label={t(lang, "pillCancel")}
          title={t(lang, "pillCancel")}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <path d="M6 6l12 12M18 6L6 18" />
          </svg>
        </button>
      )}

      <div className="rec-pill-center" role="status" aria-live="polite">
        {state === "starting" && (
          <>
            <span className="rec-pill-dot rec-pill-dot-idle" aria-hidden="true" />
            <span className="rec-pill-sr">{t(lang, "pillStarting")}</span>
            {chip}
          </>
        )}
        {(state === "recording" || state === "locked") && (
          <>
            <span className="rec-pill-dot rec-pill-dot-live" aria-hidden="true" />
            <span className="rec-pill-time">{formatElapsed(elapsed)}</span>
            {state === "locked" && (
              <svg
                className="rec-pill-lock"
                width="12"
                height="12"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2.5"
                strokeLinecap="round"
                strokeLinejoin="round"
                role="img"
                aria-label={t(lang, "pillLocked")}
              >
                <rect x="5" y="11" width="14" height="10" rx="2" />
                <path d="M8 11V7a4 4 0 0 1 8 0v4" />
              </svg>
            )}
            {chip}
          </>
        )}
        {state === "processing" && (
          <>
            <span className="rec-pill-spinner" aria-hidden="true" />
            <span className="rec-pill-text">{t(lang, "popupTranscribing")}</span>
          </>
        )}
        {state === "hint" && <span className="rec-pill-text rec-pill-text-wrap">{t(lang, "pillHint")}</span>}
        {state === "notice" && (
          <span className="rec-pill-text rec-pill-text-wrap rec-pill-text-notice" title={payload.message}>
            {payload.message}
          </span>
        )}
        {state === "error" && (
          <span className="rec-pill-text rec-pill-text-wrap rec-pill-text-error" title={payload.message}>
            {payload.message}
          </span>
        )}
      </div>

      {hasButtons &&
        (state === "processing" ? (
          <span className="rec-pill-spacer" aria-hidden="true" />
        ) : (
          <button
            type="button"
            className="rec-pill-button rec-pill-button-accept"
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => invoke("dictation_pill_accept").catch(() => {})}
            aria-label={t(lang, "pillAccept")}
            title={t(lang, "pillAccept")}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.75" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
              <path d="M5 12.5l4.5 4.5L19 7.5" />
            </svg>
          </button>
        ))}
    </div>
  );
}

export default Pill;
