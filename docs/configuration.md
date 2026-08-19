# Configuration

The application stores non-secret settings at `~/Library/Application Support/talk-to-me-goose/config.toml` on macOS when the platform configuration directory resolves to the standard location. The tray menu's `Open settings file` action opens the file for editing.

The default configuration is:

```toml
provider = "openai"
model = "gpt-4o-mini-transcribe"
shortcut = "Command+Shift+Space"
max_recording_seconds = 300
```

API keys are stored in the platform credential store. For a terminal-based setup, pipe a key to the application:

```sh
printf '%s' "$OPENAI_API_KEY" | talk-to-me-goose --set-api-key openai
printf '%s' "$GEMINI_API_KEY" | talk-to-me-goose --set-api-key gemini
```

The tray menu also provides a macOS API-key dialog. Keys are not written to `config.toml` and are not displayed in status text.

The normal workflow releases captured audio after transcription succeeds or fails. Pending transcripts are held in memory only and are lost when the application exits.

## Diagnostic Logging

The application log is stored at `~/Library/Application Support/talk-to-me-goose/app.log` on macOS. It is line-buffered and can be followed while the app is running:

```sh
tail -f "$HOME/Library/Application Support/talk-to-me-goose/app.log"
```

Entries include timestamps, severity, lifecycle state, capability names, provider and model names, operation identifiers, timing, and sanitized errors. Logs do not include API keys, audio, transcripts, clipboard contents, or full provider request and response bodies. Review application names and error details before sharing a log with someone else.

Set `RUST_LOG=debug` before launching from a terminal to increase diagnostic detail. Finder-launched instances still write to the same file.
