import { useRef, useState } from "react";
import type { Lang } from "../i18n";
import { t } from "../i18n";

function isModifierKeyCode(code: string): boolean {
  return /^(Meta|Control|Alt|Shift)(Left|Right)?$/.test(code);
}

function captureShortcut(e: React.KeyboardEvent): string | null {
  const mods: string[] = [];
  if (e.metaKey) mods.push("CommandOrControl");
  if (e.ctrlKey) mods.push("Control");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");

  if (isModifierKeyCode(e.code) || mods.length === 0) return null;
  return [...mods, e.code].join("+");
}

export function displayShortcut(raw: string): string {
  return raw
    .split("+")
    .map((token) =>
      token === "CommandOrControl"
        ? "CMD"
        : token.replace(/^Key/, "").replace(/^Digit/, "").toUpperCase(),
    )
    .join("+");
}

function ShortcutRecorder({
  lang,
  value,
  onSave,
}: {
  lang: Lang;
  value: string;
  onSave: (shortcut: string) => void;
}) {
  const [recording, setRecording] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    e.preventDefault();

    if (e.key === "Escape") {
      inputRef.current?.blur();
      return;
    }

    const combo = captureShortcut(e);
    if (combo) {
      setRecording(false);
      onSave(combo);
      inputRef.current?.blur();
    }
  };

  return (
    <input
      ref={inputRef}
      type="text"
      readOnly
      className={`pill shortcut-recorder${recording ? " recording" : ""}`}
      value={recording ? t(lang, "shortcutRecording") : displayShortcut(value)}
      onFocus={() => setRecording(true)}
      onBlur={() => setRecording(false)}
      onKeyDown={handleKeyDown}
    />
  );
}

export default ShortcutRecorder;
