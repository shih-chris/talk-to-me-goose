## Context

The repository contains only a placeholder Rust binary. The proposal and `voice-dictation` specification define a background dictation utility whose portable transcription workflow must be separated from operating-system-specific global input, microphone, credential, and text-insertion behavior.

The initial product target is macOS. Linux compatibility is an architectural constraint and a follow-on platform target, not a promise that restricted desktop environments will expose the same capabilities.

## Goals / Non-Goals

**Goals:**

- Establish a single-process desktop utility with a portable Rust core and explicit platform adapters.
- Make the recording lifecycle deterministic: idle, recording, transcribing, inserting, pending, or error.
- Support a default `primary + Shift + Space` global toggle, mapped to Command on macOS and an appropriate primary modifier on other platforms.
- Insert into the destination currently focused when transcription completes without requiring accessibility focus inspection.
- Support direct provider requests while keeping provider-specific protocols behind a common transcription boundary.
- Provide a small menu-bar or tray surface for status, configuration, permissions, and pending transcript recovery.

**Non-Goals:**

- Streaming or partial transcription in the initial workflow.
- A server-side proxy, account system, transcript history, or cloud synchronization.
- Application-specific plugins or editor integrations.
- Guaranteed global keyboard capture or automatic insertion on every Linux desktop environment.
- Rich transcript editing, formatting commands, or local model hosting.

## Decisions

### Portable Rust core with platform adapters

The application will be organized around portable session, transcription, configuration, and pending-result logic, with platform adapters for global shortcuts, audio capture, secure credentials, clipboard access, and text insertion. Platform capability checks will be explicit rather than hidden behind assumptions.

This preserves the value of the Rust scaffold without pretending that macOS and Linux provide the same desktop integration APIs. A native Swift-only application would reduce macOS integration friction but would make the desired cross-platform core less direct. A Tauri or Electron application would simplify a settings UI but add a runtime and webview boundary that is disproportionate to this utility.

### Single background process and native status surface

The first version will run as one long-lived process with a menu-bar or system-tray item. The process owns the session state and does not require a daemon, local server, or IPC protocol. The status surface will show the current lifecycle state, capability problems, provider settings, and pending transcripts without becoming the typing target or stealing focus.

### Explicit dictation state machine

The core workflow will model these states and transitions:

```text
idle --shortcut--> recording --shortcut--> transcribing
  ^                                      |
  |                                      v
  +---------- inserted / error <----- result
                                         |
                                         +--> pending when target is unsafe
```

The shortcut is ignored or reported as busy while transcription is in progress. A failed provider request does not create a transcript and returns the session to idle. A successful transcript is sent directly to the current text destination for insertion; insertion failure routes it to pending retention.

### Batch audio capture

Recording will collect audio until the second shortcut activation and send one completed request to the selected provider. Audio will be held in memory or an application-controlled temporary representation only for the duration of the request. Batch processing is preferred over streaming because it matches the requested interaction and avoids provider-specific partial-result reconciliation.

### Provider adapter boundary

The core will depend on a narrow transcription operation that accepts completed audio and model settings and returns transcript text or a normalized error. OpenAI and Gemini implementations will translate that operation into their respective authenticated HTTP requests. Credentials will be obtained from platform-secure storage, and provider configuration will remain separate from session state.

Direct client-to-provider requests are preferred over a proxy because this is a local utility and a proxy would introduce hosting, authentication, and data-routing complexity without supporting the core workflow.

### Current-focus insertion

The application will not capture or compare an accessibility focus target. The initial insertion strategy will use plain-text clipboard paste into whichever destination is focused when transcription completes, because it works across many applications and lets the target application own undo and text semantics. The original clipboard contents will be saved before insertion and restored after the paste where possible. If insertion is unavailable or fails, the transcript becomes pending. Direct accessibility value replacement remains an alternative for a later platform-specific strategy.

### Capability-based platform support

The application will expose whether each required platform capability is available: global shortcut, microphone, automatic insertion, and secure credentials. Focus inspection is not a required capability. macOS will be the first complete adapter. Linux support will reuse the core and add adapters where the desktop environment permits them; X11 and Wayland will be treated as different capability profiles because Wayland commonly restricts global input observation and synthetic input.

### In-memory pending transcripts

Every transcript that cannot be inserted will remain in an in-memory pending collection until copied or dismissed. Copying deliberately places the selected transcript on the system clipboard. Pending transcripts are not required to survive application exit in the initial version, avoiding silent persistence of potentially sensitive speech-derived content.

## Risks / Trade-offs

- [macOS permissions and packaging] Global input monitoring, accessibility, microphone access, signing, and notarization can fail independently → expose each capability separately and validate them with a packaged application early.
- [Linux desktop restrictions] Wayland may prevent both the global shortcut and automatic insertion → report unavailable capabilities and provide transcription plus copy as the fallback instead of emulating unsafe behavior.
- [Current-focus changes] The user may switch applications while transcription is in flight, causing text to be inserted into the newly focused destination → accept this behavior for the simple workflow and provide pending recovery only when insertion itself fails.
- [Clipboard races] Restoring the clipboard immediately after synthetic paste may race with the target application or overwrite a user change → isolate clipboard handling, restore only after the paste operation is dispatched, and avoid restoration after an explicit user clipboard change where detectable.
- [Provider differences] Audio formats, limits, errors, and model options vary by provider → keep provider translation at the adapter boundary and normalize only the behavior needed by the session workflow.
- [Network latency and long recordings] A request may take long enough for the user to change context or consume significant memory → show transcribing status, insert into the destination current at completion, and impose a documented initial recording limit.
- [Shortcut conflicts] The default shortcut may already be registered by the operating system or another application → report registration failure and allow a configurable shortcut.

## Migration Plan

There is no existing application behavior or persisted data to migrate. Implementation can replace the placeholder binary directly. Development should validate the macOS capability probes and insertion path before packaging the full provider and settings experience. If the change is rolled back, removing the new application binary and its configuration is sufficient; no database or server rollback is required.

## Open Questions

- Which Rust tray/menu-bar and platform integration libraries provide the best packaging behavior once the macOS capability spike is complete?
- Should launch-at-login be included in the first packaged release or added after the core workflow is stable?
- What initial recording duration and audio encoding provide the best balance between provider compatibility, latency, and memory use?
