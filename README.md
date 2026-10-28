<img src="src-tauri/icons/icon.png" alt="TypeLang icon" width="96" height="96">

# TypeLang

A tiny menu-bar app for macOS that translates what you're typing — anywhere — into natural, professional English (or whatever language pair you set), without leaving the app you're working in.

Press a global keyboard shortcut, a small floating window appears wherever your cursor is, type in your language, hit Enter — the natural translation is pasted directly into the field you were writing in. No copy-pasting into a browser tab, no breaking your flow.

![TypeLang translating a line of text via its floating popup](example-usage.gif)

> **New in 0.2.0:** voice dictation (🎤 in the popup, or hold `Alt+Shift+D` anywhere), custom vocabulary, snippets, optional autostart of local AI servers, and a fix for the paste that sometimes typed a single letter instead of the translation. Upgrading from 0.1.0? See the [upgrade notes](CHANGELOG.md#upgrading-from-010) — you'll need to re-grant Accessibility once. Full list in the [CHANGELOG](CHANGELOG.md).

## Why

Non-literal translation. If you speak, say, Spanish and work with English-speaking colleagues, tools like Google Translate produce stiff, word-for-word English. TypeLang sends your text to an LLM (Claude, GPT, Gemini, or a local model through any OpenAI-compatible server such as LM Studio or llama.cpp) with a prompt tuned for natural, idiomatic, professional phrasing — the way a fluent bilingual coworker would actually write it.

You can also **speak** instead of type: dictate in your language and get polished English pasted where your cursor is — like Wispr Flow, but running entirely on your own machine if you want it to.

## Features

- **Global shortcut, configurable** — record any key combination in Settings; defaults to something unlikely to collide with your browser's shortcuts.
- **Bring your own API key (BYOK)** — plug in Anthropic, OpenAI, Google Gemini, or point it at any local OpenAI-compatible server ([LM Studio](https://lmstudio.ai), [llama.cpp](https://github.com/ggml-org/llama.cpp)'s `llama-server`, …). No account, no subscription, no data going through a third-party server you don't control.
- **Voice dictation** — click the 🎤 in the popup, speak, click again: the transcript appears in the popup so you can review it, then Enter translates it. Filler words ("eh", "o sea"…) and self-corrections ("el martes, no perdón, el miércoles") are cleaned up in the translation.
- **Hold-to-talk shortcut** — hold the dictation shortcut (default `Alt+Shift+D`), speak, release: the result is pasted straight at your cursor, no popup. **Double-tap** it instead to keep recording hands-free, and press it once more to finish. A small floating pill at the bottom of the screen shows the timer, with ✓ to finish and paste and ✕ to discard; it never takes focus from the app you are typing in. Recordings stop on their own after 5 minutes. Choose in Settings whether it pastes the translation or your original words. The menu-bar icon shows `●` while recording, `…` while processing and `⚠` on errors (hover it for details).

  ![The recording pill in hands-free mode: ✕ to discard, timer, ✓ to finish and paste](pill-recording.png)
- **Custom vocabulary** — product names, people, jargon: listed once in Settings, spelled right in both the transcript and the translation.
- **Snippets** — press ⌘Enter instead of Enter to save a translation as a snippet; a second shortcut (default `Alt+Shift+S`) opens a searchable picker to paste it again without re-translating.
- **Start local services automatically** — optionally run a command when TypeLang opens (e.g. a script that starts your local speech-to-text and LLM servers) and stop it when TypeLang quits. If the services are already running, TypeLang leaves them alone.
- **Configurable translation direction** — pick source/target languages from Settings, not hardcoded to one pair.
- **Bilingual interface** — Spanish and English UI, switchable in Settings.
- **Auto-paste, not copy-paste** — the translation lands directly in the field you were typing in, with your original clipboard contents restored afterward.
- **Quick tone toggle** — switch between professional and casual right from the tray menu (sets the default) or per-message from the popup itself (Tab, or click the tone badge for a dropdown) — handy for jumping between work chats and personal messages without opening Settings.
- **Menu-bar only** — no Dock icon, stays out of the way until you need it.

![Switching between professional and casual tone from the popup](tone-toggle.gif)

## Status

Early, actively developed, **macOS only** for now (Windows/Linux support is on the roadmap — the paste/focus-capture logic is platform-specific and hasn't been ported yet). Expect rough edges.

## Installing the built app

Build it with `npm run tauri build` (see below), then drag `src-tauri/target/release/bundle/macos/TypeLang.app` into `/Applications`.

This project doesn't (and won't) pay for an Apple Developer ID certificate — it's free and open source, and that costs $99/year for something that only silences a warning. Because the app isn't signed, **macOS Gatekeeper will refuse to open it on the first launch** with a message like "TypeLang can't be opened because Apple cannot check it for malicious software" or "...is from an unidentified developer." This is expected, not a bug. Pick one:

1. **Right-click (or Control-click) the app → Open**, instead of double-clicking. A dialog appears with an actual "Open" button this time — click it. You only need to do this once; it launches normally from then on.
2. **System Settings → Privacy & Security**, scroll down — you'll see "TypeLang was blocked to protect your Mac" with an **Open Anyway** button.
3. **Terminal, for repeat installs/updates:**
   ```sh
   xattr -d com.apple.quarantine /Applications/TypeLang.app
   ```
   This removes the quarantine flag that triggers the Gatekeeper check in the first place.

## Usage

Applies whether you're running the built `.app` or `npm run tauri dev` — the app is menu-bar only, so look for its icon next to the clock.

1. **First launch:** macOS will ask for **Accessibility** permission — this is required to simulate the paste keystroke. Grant it in System Settings → Privacy & Security → Accessibility (you'll be prompted automatically the first time you try to translate something if you haven't granted it yet).
2. **Configure a provider:** click the menu-bar icon → **Configuración/Settings** → pick a provider (Anthropic, OpenAI, Google Gemini, or LM Studio) → paste in an API key (LM Studio doesn't need one — just its server URL, e.g. `http://localhost:1234/v1`) → save.
3. **Optional settings:** interface language, translation direction (which language you type in → which it translates to), and the global keyboard shortcut — click the shortcut field and press a new combination to change it.
4. **Translate:** place your cursor in any text field, press the shortcut (default `Alt+Shift+T`), type in your source language, press **Enter**. The popup closes and the translation is pasted automatically where your cursor was. Press **Escape** to cancel without pasting anything. The ⚙️ icon in the popup jumps straight to Settings.
5. **Dictate:** in the popup, click 🎤, speak, click 🎤 again — review the transcript and press Enter. Or hold the dictation shortcut (default `Alt+Shift+D`) anywhere, speak, and release — or tap it twice quickly to keep recording hands-free and press it again to finish (the pill's ✓ finishes, its ✕ discards; a single quick tap is ignored). The first time, macOS asks for **Microphone** permission; the permission dialog takes focus and closes the popup, so just reopen it and try again. Escape cancels a recording in the popup; popup recordings stop on their own after 2 minutes, shortcut recordings after 5.

### Running everything locally

Dictation needs a speech-to-text server with an OpenAI-compatible `/v1/audio/transcriptions` endpoint (Settings → Dictation, default `http://127.0.0.1:8178/v1`); translation needs an LLM server (Settings → provider "LM Studio", any OpenAI-compatible URL). A setup that works well on Apple Silicon:

```sh
# Speech-to-text: whisper.cpp (brew install whisper-cpp ffmpeg)
# Run it from a writable directory — --convert writes a temporary file there.
cd ~/Library/Caches && whisper-server \
  -m ggml-large-v3-turbo-q5_0.bin -l es \
  --host 127.0.0.1 --port 8178 \
  --inference-path /v1/audio/transcriptions --convert

# Translation: llama.cpp with Gemma 4 E4B (Q4_K_M, ~5 GB), thinking disabled
llama-server -m gemma-4-E4B-it-Q4_K_M.gguf --alias gemma-4-e4b \
  --host 127.0.0.1 --port 8091 --jinja -ngl 99 -c 4096 \
  --chat-template-kwargs '{"enable_thinking":false}'
```

Then in Settings use `http://127.0.0.1:8091/v1` with model `gemma-4-e4b`. Thinking must be off on the server: TypeLang doesn't send reasoning flags, and a thinking model can spend its whole budget reasoning and return an empty answer. On an M4 Max this gives ~0.5 s transcription and ~0.3 s translation per sentence, and Gemma 4 E4B handled the tricky cases in our tests (who-does-what pronouns, mid-sentence self-corrections) better than other small models we tried.

To have TypeLang start these for you, put the commands in a script and set it under Settings → Local services. If you point the dictation URL at another machine, Settings warns you that your voice will be sent there.

## Development

Requirements: [Rust](https://rustup.rs), Node.js, and on macOS, Xcode Command Line Tools.

```sh
npm install
npm run tauri dev
```

This runs the app with hot-reload (frontend changes apply instantly; Rust changes trigger an automatic rebuild). Same first-run Accessibility prompt and Settings flow as above.

To build the standalone `.app` described above instead of running in dev mode:

```sh
npm run tauri build
```

The built app lands at `src-tauri/target/release/bundle/macos/TypeLang.app`.

### Keeping permissions across rebuilds (local signing)

By default the app is ad-hoc signed, and macOS ties Accessibility, Microphone and Local Network permissions to the exact build — so every rebuild asks for them again. Signing with a self-signed certificate makes macOS recognise TypeLang by its bundle id + your certificate instead, and the permissions stick:

1. Keychain Access → Certificate Assistant → **Create a Certificate…** → name it e.g. `TypeLang Local Signing`, Identity Type **Self Signed Root**, Certificate Type **Code Signing**.
2. Build with that identity (no config change needed):
   ```sh
   APPLE_SIGNING_IDENTITY="TypeLang Local Signing" npm run tauri build
   ```

Signing with a real identity also turns on macOS's hardened runtime, which blocks the microphone unless the app declares it. `src-tauri/Entitlements.plist` (`com.apple.security.device.audio-input`) does that; keep it if you change the bundle config, or dictation will record silence.

This only helps on your own Mac; anyone else running the app still sees the "unidentified developer" warning.

### Tests

```sh
cd src-tauri && cargo test
```

## How it works

- **Rust/Tauri backend** — registers the global shortcuts, captures whichever app has focus (by bundle id), shows a floating popup window, calls the configured LLM provider, then restores focus and pastes. The paste is a single CoreGraphics ⌘V event with the Command flag set on the event itself, posted only after you've released your modifier keys — so it can't arrive as a bare "v". If TypeLang can't get back to the original app, it shows an error instead of pasting somewhere else, and it only restores your previous clipboard if nothing else was copied in the meantime.
- **Dictation** — the microphone is captured in Rust ([`cpal`](https://github.com/RustAudio/cpal)) on a dedicated thread, encoded as WAV in memory ([`hound`](https://github.com/ruuda/hound)) and sent to the speech-to-text server; nothing is written to disk or logged. Popup recordings are capped at 2 minutes and cancelled whenever the popup closes; shortcut recordings are capped at 5 minutes.
- **Recording pill** — on macOS the pill is a non-activating `NSPanel` (via [`tauri-nspanel`](https://github.com/ahkohd/tauri-nspanel), which requires `macOSPrivateApi` in `tauri.conf.json`), so clicking it never takes focus from the app you're dictating into. The hold / double-tap logic is a pure state machine in `src-tauri/src/dictation_machine.rs`.
- **React/TypeScript frontend** — the Settings window, the floating translate popup and the recording pill, all built from the same Vite bundle and routed by URL hash.
- API keys live in the OS keychain (via the [`keyring`](https://github.com/hwchen/keyring-rs) crate), never in plain-text config.

## License

MIT — see [LICENSE](./LICENSE).

## Contributing

This is a young, personal project — issues and PRs are welcome, but expect the architecture to shift as it matures.
