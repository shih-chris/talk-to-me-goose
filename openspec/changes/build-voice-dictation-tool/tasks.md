## 1. Foundation and Capability Boundaries

- [x] 1.1 Evaluate Rust desktop, global-shortcut, audio, clipboard, and secure-storage options and record the selected platform support assumptions.
- [x] 1.2 Replace the placeholder binary with portable modules for session state, transcription requests, configuration, pending transcripts, and normalized errors.
- [x] 1.3 Define platform capability interfaces for global shortcuts, microphone capture, credential storage, clipboard access, and text insertion.
- [x] 1.4 Add configuration validation for the provider, model, shortcut, and recording limits without exposing secret values in logs or status output.

## 2. macOS Desktop Integration

- [x] 2.1 Create the background menu-bar application surface with lifecycle status, capability status, settings access, and quit behavior.
- [x] 2.2 Implement microphone permission checks and batch audio capture for the recording lifecycle, including a documented initial duration limit.
- [x] 2.3 Implement the default `Command + Shift + Space` global shortcut as a start/stop toggle and report registration conflicts or permission failures.
- [x] 2.4 Remove start-time focus capture and insert into the current destination when transcription completes, retaining text only when insertion fails or is unavailable.
- [x] 2.5 Implement plain-text clipboard insertion without activating the application UI, restoring the previous clipboard after successful paste where possible.
- [x] 2.6 Add macOS secure credential storage for provider API keys and capability reporting for microphone, shortcut, and insertion permissions.

## 3. Transcription Providers

- [x] 3.1 Implement the provider adapter contract for completed audio, model settings, normalized transcript results, and actionable errors.
- [x] 3.2 Implement the OpenAI transcription adapter with request encoding, authentication, timeout handling, and response validation.
- [x] 3.3 Implement the Gemini transcription adapter with request encoding, authentication, timeout handling, and response validation.
- [x] 3.4 Add provider selection and model configuration to the application settings flow without displaying full API keys.

## 4. Session and Recovery Workflow

- [x] 4.1 Connect shortcut, recorder, provider, current-focus insertion, and error handling through the explicit session state machine.
- [x] 4.2 Ensure focus changes during transcription do not cancel insertion and that insertion failures add the generated transcript to the pending collection.
- [x] 4.3 Add menu-bar actions to copy or dismiss each pending transcript and communicate copy readiness.
- [x] 4.4 Ensure captured audio is released after successful or failed transcription and pending transcripts remain in memory only for the running process.
- [x] 4.5 Add busy-state handling so shortcut activation during transcription does not start another recording.

## 5. Cross-Platform Readiness

- [x] 5.1 Keep portable core tests and builds independent of macOS-only APIs through explicit platform adapters.
- [x] 5.2 Add non-macOS capability reporting that identifies unavailable global shortcut or automatic insertion behavior instead of failing silently.
- [x] 5.3 Document Linux X11 and Wayland capability differences and the copy-only fallback when automatic insertion is unavailable.

## 6. Verification and Packaging

- [x] 6.1 Add unit tests for recording toggle transitions, busy handling, provider failures, current-focus insertion, pending retention, and capability failures.
- [x] 6.2 Add provider contract tests using sanitized fixtures for successful responses, malformed responses, authentication errors, and timeouts.
- [ ] 6.3 Manually verify the packaged macOS workflow in representative text fields, including a terminal, browser text field, and editor.
- [ ] 6.4 Manually verify microphone, input-monitoring/accessibility, clipboard, focus-change, and shortcut-conflict permission paths.
- [x] 6.5 Document local development, provider configuration, required permissions, packaging, and known Linux limitations.
