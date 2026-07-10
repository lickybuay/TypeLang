# TypeLang

A tiny menu-bar app for macOS that translates what you're typing — anywhere — into natural, professional English (or whatever language pair you set), without leaving the app you're working in.

Press a global keyboard shortcut, a small floating window appears wherever your cursor is, type in your language, hit Enter — the natural translation is pasted directly into the field you were writing in. No copy-pasting into a browser tab, no breaking your flow.

## Why

Non-literal translation. If you speak, say, Spanish and work with English-speaking colleagues, tools like Google Translate produce stiff, word-for-word English. TypeLang sends your text to an LLM (Claude, GPT, Gemini, or a local model via LM Studio) with a prompt tuned for natural, idiomatic, professional phrasing — the way a fluent bilingual coworker would actually write it.

## Features

- **Global shortcut, configurable** — record any key combination in Settings; defaults to something unlikely to collide with your browser's shortcuts.
- **Bring your own API key (BYOK)** — plug in Anthropic, OpenAI, Google Gemini, or point it at a local [LM Studio](https://lmstudio.ai) server. No account, no subscription, no data going through a third-party server you don't control.
- **Configurable translation direction** — pick source/target languages from Settings, not hardcoded to one pair.
- **Bilingual interface** — Spanish and English UI, switchable in Settings.
- **Auto-paste, not copy-paste** — the translation lands directly in the field you were typing in, with your original clipboard contents restored afterward.
- **Menu-bar only** — no Dock icon, stays out of the way until you need it.

## Status

Early, actively developed, **macOS only** for now (Windows/Linux support is on the roadmap — the paste/focus-capture logic is platform-specific and hasn't been ported yet). Expect rough edges.

## Getting started (development)

Requirements: [Rust](https://rustup.rs), Node.js, and on macOS, Xcode Command Line Tools.

```sh
npm install
npm run tauri dev
```

On first run, macOS will ask for **Accessibility** permission — this is required to simulate the paste keystroke. Grant it in System Settings → Privacy & Security → Accessibility.

Then open **Settings** from the menu-bar icon, pick a translation provider, and paste in an API key (or point it at a local LM Studio server). Press the configured shortcut (default `Alt+Shift+T`) from any text field to try it.

## How it works

- **Rust/Tauri backend** — registers the global shortcut, captures whichever app has focus, shows a floating popup window, calls the configured LLM provider, then restores focus and simulates a paste keystroke via [`enigo`](https://github.com/enigo-rs/enigo).
- **React/TypeScript frontend** — the Settings window and the floating translate popup, both built from the same Vite bundle and routed by URL hash.
- API keys live in the OS keychain (via the [`keyring`](https://github.com/hwchen/keyring-rs) crate), never in plain-text config.

## License

MIT — see [LICENSE](./LICENSE).

## Contributing

This is a young, personal project — issues and PRs are welcome, but expect the architecture to shift as it matures.
