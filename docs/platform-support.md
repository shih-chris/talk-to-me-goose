# Platform Support

## macOS

macOS is the first complete target. The application uses `Command + Shift + Space` as the default global shortcut, CoreAudio for microphone capture, the Keychain for provider credentials, and clipboard paste for automatic insertion into the destination currently focused when transcription completes.

The packaged application must include the microphone usage description in `Info.plist`. The user must also grant Accessibility or Input Monitoring access when macOS prompts for global shortcut or synthetic keyboard permissions.

## Linux X11

The portable core and audio/provider layers can be reused on Linux. X11 can support global shortcut registration and synthetic paste when the required desktop libraries are available. A Linux adapter should report each capability independently because desktop configurations vary.

## Linux Wayland

Wayland intentionally restricts global keyboard observation and synthetic keyboard input. The application must not claim that automatic insertion works when those capabilities are unavailable. The supported fallback is to transcribe the recording, retain the result as pending text, and let the user copy it from the tray UI.

## Capability Reporting

The tray menu reports the status of the shortcut, microphone, automatic insertion, and credential store separately. An unavailable capability is a supported state and should produce an actionable message rather than a silent failure.
