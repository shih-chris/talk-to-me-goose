## Why

The background application currently provides too little visibility when a global shortcut, permission check, menu action, audio capture, current-destination insertion, or provider request fails. Persistent diagnostic logs will make Finder-launched instances debuggable without requiring a terminal or exposing sensitive dictation data.

## What Changes

- Add application logging initialized during startup and written to a discoverable local log file.
- Record lifecycle events for startup, configuration loading, platform capability checks, menu actions, global shortcut registration and events, recording, transcription, current-destination insertion, pending recovery, and shutdown.
- Include timestamps, severity, and useful non-sensitive context in each log entry.
- Keep logs useful in real time by flushing entries promptly and documenting how to follow the log while the application runs.
- Redact API keys and avoid logging captured audio, transcript text, clipboard contents, or other sensitive payloads.
- Record permission and capability failures with actionable context, including the capability that is unavailable.

## Capabilities

### New Capabilities

- `diagnostic-logging`: Persistent, near-real-time, privacy-safe application diagnostics for troubleshooting desktop integration and transcription workflow failures.

### Modified Capabilities

None.

## Impact

The desktop application will gain a logging dependency and write a local application log under the platform's standard user data or logs directory. Existing workflow code will emit structured diagnostic events at platform, session, provider, and UI boundaries. Log files may contain application names, capability states, error messages, model names, and timing information, but must not contain credentials, audio, transcripts, or clipboard contents.
