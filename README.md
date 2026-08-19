# Talk To Me Goose

Talk To Me Goose is a focused desktop voice-to-text utility. Press the global shortcut to start recording, press it again to stop, and the resulting transcript is inserted into the text destination that is focused when transcription completes.

## Current Platform Plan

The portable Rust core owns session state, provider requests, configuration, pending transcripts, and normalized errors. Platform adapters own global shortcuts, microphone capture, credentials, clipboard access, and text insertion.

The first complete adapter targets macOS with `Command + Shift + Space`. Linux support is capability-aware: X11 can support more of the workflow, while Wayland may restrict global shortcuts and synthetic text insertion. Unsupported capabilities fall back to transcription plus a copy action.

## Development

Run `cargo test` for the portable core tests and `cargo run` to start the tray application. The application requires microphone permission and accessibility/input-monitoring permissions on macOS before recording and automatic insertion can work.

Provider settings are configured from the tray menu. API credentials are stored in the platform credential store and are never written to the application configuration file.

Diagnostic logs are written to `~/Library/Application Support/talk-to-me-goose/app.log` on macOS. Follow them with `tail -f "$HOME/Library/Application Support/talk-to-me-goose/app.log"`; logs exclude API keys, audio, transcripts, and clipboard contents.
