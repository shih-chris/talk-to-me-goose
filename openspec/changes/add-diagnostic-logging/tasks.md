## 1. Logging Foundation

- [x] 1.1 Add the tracing dependencies and create a logging module with the documented platform data path.
- [x] 1.2 Initialize a line-buffered append logger before platform services and the event loop, with a non-fatal stderr fallback.
- [x] 1.3 Record startup version, process identifier, log path, configuration load result, and logging fallback state without logging secrets.
- [x] 1.4 Support `RUST_LOG` severity filtering while defaulting to concise informational logging.

## 2. Workflow Instrumentation

- [x] 2.1 Log platform capability checks and permission or registration failures with capability names and sanitized error summaries.
- [x] 2.2 Log menu action identifiers, global shortcut registration outcomes, and received hotkey event states.
- [x] 2.3 Log session state transitions, recording start/stop metadata, audio capture failures, and transcription timing without audio or transcript content.
- [x] 2.4 Log current-destination insertion outcomes and pending transcript copy or dismissal without text or clipboard contents.
- [x] 2.5 Log provider selection and request lifecycle metadata, including provider, model, duration, status, and sanitized error category without keys or payloads.
- [x] 2.6 Log application shutdown and ensure all event paths use the same redaction policy.

## 3. Documentation and Verification

- [x] 3.1 Document the macOS log path, platform-relative path pattern, real-time follow command, expected fields, and sharing precautions.
- [x] 3.2 Add tests for logger path resolution, startup fallback behavior, severity filtering, and sensitive-value redaction boundaries.
- [ ] 3.3 Verify the packaged Finder-launched app creates and flushes the log while menu and hotkey actions are reproduced.
- [ ] 3.4 Verify logs remain free of API keys, audio, transcripts, clipboard contents, and full provider request or response bodies.
