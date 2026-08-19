## Context

The application is a single-process Rust menu-bar utility with platform adapters for macOS input, audio, focus, credentials, clipboard, and insertion. It is often launched from Finder, so standard output is not a dependable debugging surface. See `proposal.md` and `specs/diagnostic-logging/spec.md` for the motivation and observable behavior.

## Goals / Non-Goals

**Goals:**

- Provide a human-readable log file that can be followed while the app is running.
- Initialize logging before platform services and the event loop so startup failures are visible.
- Make each major event boundary observable without adding logging-specific control flow to the session state machine.
- Preserve privacy by logging metadata and sanitized errors rather than payloads.
- Keep logging failure non-fatal and surface the fallback mode when the normal file cannot be used.

**Non-Goals:**

- Remote log collection, analytics, crash reporting, or automatic upload.
- Logging raw microphone data, transcripts, clipboard contents, provider request bodies, or API credentials.
- A user-facing log viewer or log search UI in the first version.
- Debug-level logging enabled permanently by default if it would materially increase log volume.

## Decisions

### Use the Rust tracing ecosystem with a line-buffered file writer

Use `tracing` event macros throughout the application and `tracing-subscriber` for formatting and filtering. Write human-readable entries through a line-buffered file writer so each event is visible promptly to `tail -f` or an equivalent log viewer. Default to `info` severity and allow `RUST_LOG` to increase verbosity for local debugging.

This is preferred over scattered `println!` calls because it provides consistent timestamps, levels, filtering, and a safe path to structured fields. A JSON-only logger is deferred because the primary consumer is a person debugging a local desktop app.

### Use the standard per-user application data location

Place the log at the platform data directory under `talk-to-me-goose/app.log`, which resolves to the macOS Application Support directory used by the existing configuration. Document the concrete macOS path and the platform-relative pattern. The logger will append to the file between launches so startup history is retained.

The initial implementation will not add rotation or remote retention. The log writer and documentation should make clearing the file straightforward, and volume should remain bounded by logging lifecycle events rather than audio or text payloads.

### Initialize once and retain fallback state

Startup will attempt to create the log directory, open the file in append mode, and install the subscriber before creating the event loop. If that fails, it will install a stderr subscriber where possible and retain a short logging-status value for the application status UI. Logging failure must never prevent the application from attempting its normal workflow.

### Log at boundaries, not inside sensitive payload handling

Add events at the application boundary points:

```text
startup/config/capabilities
        |
        v
menu or hotkey -> session state -> audio -> provider
                         |              |
                         v              v
                 focus/insertion   sanitized outcome
                         |
                         v
                    pending UI
```

Fields may include operation, provider, model, shortcut registration result, capability name, state, duration, byte/sample counts, application identifier, and sanitized error category. Fields must not include text values or raw request/response data. Provider error handling will log status and a bounded safe summary only after removing payload-bearing details where needed.

### Use an explicit redaction policy

The logging policy will be documented in code near the initialization and logging helpers. API keys will never be passed to a logging macro. Transcript, clipboard, and audio variables will not be interpolated into messages. Error logs will use operation-specific summaries rather than blindly formatting provider response bodies.

## Risks / Trade-offs

- [Unbounded append file] A long-running installation can accumulate history → keep default event volume low, document the path, and add rotation as a follow-up if real usage shows growth.
- [Fallback visibility] Finder-launched stderr may not be visible → expose the intended log path and logging status through the menu UI when possible.
- [Sensitive error text] Third-party errors can contain request details → avoid raw response logging and centralize bounded error summarization.
- [Logging overhead] Synchronous file writes can briefly affect the event loop → use line buffering and short metadata-only entries; revisit an asynchronous writer if profiling shows impact.
- [Multiple application instances] Two instances may append concurrently → rely on append-mode OS writes for individual lines and log the process identifier to distinguish sessions.

## Migration Plan

No persisted application data or API changes require migration. Add the logging dependency and initialize it during startup. Existing users will receive a new log file on the next launch; no configuration changes are required. Rollback consists of removing the logging initialization and dependency, leaving existing configuration and credentials untouched.

## Open Questions

- Whether log rotation is needed can be decided after observing real log volume and does not change the initial logging contract.
