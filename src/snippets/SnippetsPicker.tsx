import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { t } from "../i18n";
import type { Lang } from "../i18n";
import "./snippets.css";

type Snippet = {
  id: string;
  label: string;
  text: string;
};

type SettingsView = {
  ui_language: string;
};

const WINDOW_WIDTH = 320;
const MIN_WINDOW_HEIGHT = 160;
const MAX_WINDOW_HEIGHT = 420;
// Same rationale as the translate popup's WINDOW_CHROME — room for the
// window's own padding plus the card's drop-shadow, not accounted for by
// the card's own scrollHeight.
const WINDOW_CHROME = 36;

function SnippetsPicker() {
  const [snippets, setSnippets] = useState<Snippet[]>([]);
  const [query, setQuery] = useState("");
  const [activeIndex, setActiveIndex] = useState(0);
  const [lang, setLang] = useState<Lang>("en");
  const inputRef = useRef<HTMLInputElement>(null);
  const cardRef = useRef<HTMLDivElement>(null);
  const [revealKey, setRevealKey] = useState(0);

  const filtered = snippets.filter((s) => {
    const q = query.toLowerCase();
    return s.label.toLowerCase().includes(q) || s.text.toLowerCase().includes(q);
  });

  const resizeToContent = () => {
    const card = cardRef.current;
    if (!card) return;
    const desired = card.scrollHeight + WINDOW_CHROME;
    const clamped = Math.min(MAX_WINDOW_HEIGHT, Math.max(MIN_WINDOW_HEIGHT, desired));
    getCurrentWindow()
      .setSize(new LogicalSize(WINDOW_WIDTH, clamped))
      .catch(() => {});
  };

  const fetchAll = async () => {
    try {
      const [list, s] = await Promise.all([
        invoke<Snippet[]>("get_snippets"),
        invoke<SettingsView>("get_settings"),
      ]);
      setSnippets(list);
      setLang(s.ui_language === "es" ? "es" : "en");
    } catch {
      // Non-fatal — picker still opens, just possibly empty/stale.
    }
  };

  const reset = () => {
    setQuery("");
    setActiveIndex(0);
    setRevealKey((k) => k + 1);
    inputRef.current?.focus();
    fetchAll();
  };

  useEffect(() => {
    inputRef.current?.focus();
    fetchAll();
    const unlisten = listen("snippets-reset", reset);
    return () => {
      unlisten.then((f) => f());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    resizeToContent();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [filtered.length, query]);

  useEffect(() => {
    setActiveIndex(0);
  }, [query]);

  const choose = (snippet: Snippet) => {
    invoke("paste_snippet", { text: snippet.text }).catch(() => {
      // Non-fatal — window stays open (Rust only hides it on success), so
      // the user can just try again.
    });
  };

  const cancel = () => {
    invoke("cancel_snippets");
  };

  return (
    <div
      className="snippets"
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.preventDefault();
          cancel();
        } else if (e.key === "ArrowDown") {
          e.preventDefault();
          setActiveIndex((i) => Math.min(i + 1, filtered.length - 1));
        } else if (e.key === "ArrowUp") {
          e.preventDefault();
          setActiveIndex((i) => Math.max(i - 1, 0));
        } else if (e.key === "Enter") {
          e.preventDefault();
          const chosen = filtered[activeIndex];
          if (chosen) choose(chosen);
        }
      }}
    >
      <div className="snippets-card" ref={cardRef} key={revealKey}>
        <input
          ref={inputRef}
          className="pill snippets-search"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder={t(lang, "snippetsSearchPlaceholder")}
        />
        <div className="snippets-list">
          {filtered.length === 0 ? (
            <div className="snippets-empty">
              {snippets.length === 0 ? t(lang, "snippetsEmptyState") : t(lang, "snippetsNoMatches")}
            </div>
          ) : (
            filtered.map((s, i) => (
              <button
                type="button"
                key={s.id}
                className={`snippets-item${i === activeIndex ? " active" : ""}`}
                onMouseEnter={() => setActiveIndex(i)}
                onClick={() => choose(s)}
              >
                {s.label}
              </button>
            ))
          )}
        </div>
        <div className="snippets-hint">{t(lang, "snippetsPickerHint")}</div>
      </div>
    </div>
  );
}

export default SnippetsPicker;
