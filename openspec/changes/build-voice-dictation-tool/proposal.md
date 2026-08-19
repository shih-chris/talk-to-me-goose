## Why

macOS keyboard dictation is not sufficiently configurable for a focused developer workflow. This change introduces a background voice-to-text utility that can be invoked from any text field, send a recording to a user-selected transcription provider, and safely place the resulting text at the destination currently focused when transcription completes.

The repository is currently an empty Rust scaffold, making this a clean point to establish a portable core while isolating operating-system-specific input, audio, focus, and text-insertion behavior.

## What Changes

- Add a background menu-bar/tray application with a configurable global `Command + Shift + Space` toggle on macOS.
- Start microphone recording on the first shortcut activation and stop recording on the second activation.
- Transcribe completed recordings through a provider abstraction supporting configured services such as OpenAI and Gemini.
- Insert the transcript into whichever text destination is focused when transcription completes without requiring focus inspection or a focus snapshot at recording start.
- Use safe text insertion that restores the clipboard after successful insertion where possible.
- Retain transcripts whose insertion is unavailable or fails so the user can copy them from the application UI.
- Structure portable transcription and session logic separately from macOS and future Linux integrations.
- Detect and communicate unavailable platform capabilities rather than promising identical behavior on restricted environments such as Linux Wayland.

## Capabilities

### New Capabilities

- `voice-dictation`: Global recording control, provider-backed transcription, current-focus insertion, pending transcript retention, configuration, and platform capability reporting.

### Modified Capabilities

None.

## Impact

This will replace the placeholder Rust binary with a long-running desktop application and add dependencies for audio capture, HTTP provider clients, secure configuration storage, global shortcuts, clipboard/text insertion, and the platform tray or menu-bar shell. The application will require microphone, global input, and text-insertion permissions on macOS. Linux support will share portable behavior but may require platform-specific adapters and may be limited under Wayland.
