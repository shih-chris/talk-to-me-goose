## Purpose

Provide a focused, background voice-to-text workflow that lets a user dictate into the text destination currently focused when transcription completes without depending on integrations with individual applications.

## ADDED Requirements

### Requirement: The application SHALL provide a global recording toggle

The application SHALL run as a background menu-bar or system-tray utility and SHALL expose a global shortcut that starts and stops a recording session. The default macOS shortcut SHALL be `Command + Shift + Space`, and the shortcut SHALL be configurable when the platform supports global shortcut registration.

#### Scenario: Starting a recording session
- **WHEN** the user activates the configured shortcut while the application is idle
- **THEN** the application SHALL request microphone access if needed, begin recording, and expose a recording status without taking focus from the target application

#### Scenario: Stopping a recording session
- **WHEN** the user activates the configured shortcut while the application is recording
- **THEN** the application SHALL stop recording and begin transcription of the captured audio

#### Scenario: Shortcut activated while transcribing
- **WHEN** the user activates the configured shortcut while a stopped recording is being transcribed
- **THEN** the application SHALL not start a second recording and SHALL communicate that the current session is still processing

#### Scenario: Required recording capability is unavailable
- **WHEN** the user attempts to start recording without an available microphone or required permission
- **THEN** the application SHALL remain idle, explain the unavailable capability, and provide a way to resolve or inspect the permission state

### Requirement: The application SHALL transcribe completed recordings through a configured provider

After recording stops, the application SHALL send the captured audio to the configured voice-to-text provider and SHALL produce a transcript or an actionable error. The provider choice and model configuration SHALL support at least OpenAI and Gemini integrations without exposing provider-specific request formats to the rest of the workflow.

#### Scenario: Successful transcription
- **WHEN** a recording is stopped and the configured provider successfully processes it
- **THEN** the application SHALL make the resulting transcript available to the insertion workflow

#### Scenario: Transcription failure
- **WHEN** the configured provider rejects the request, times out, or returns an invalid response
- **THEN** the application SHALL not insert partial or unknown text, SHALL return to an idle state, and SHALL communicate an actionable error

#### Scenario: Provider credentials are unavailable
- **WHEN** transcription is requested without valid credentials or required provider configuration
- **THEN** the application SHALL not upload the recording, SHALL communicate the missing configuration, and SHALL return to an idle state

### Requirement: The application SHALL insert into the current text destination

The application SHALL not require a focus snapshot or focused-element identity when recording begins. After transcription completes, it SHALL attempt to insert the transcript into whichever text destination is currently focused, without activating or focusing the application UI. If insertion is unavailable or fails, the application SHALL retain the transcript as pending text.

#### Scenario: Current text destination is focused
- **WHEN** transcription completes and a text destination is currently focused
- **THEN** the application SHALL insert the transcript into that current destination without inspecting whether it was focused when recording began

#### Scenario: Focus changes during processing
- **WHEN** transcription completes after the user changes applications or text destinations
- **THEN** the application SHALL attempt insertion into the newly current destination

#### Scenario: Current insertion is unavailable
- **WHEN** transcription completes and clipboard or automatic insertion is unavailable or fails
- **THEN** the application SHALL not claim successful insertion and SHALL retain the transcript as pending text

### Requirement: The application SHALL provide pending transcript recovery

The application SHALL retain every successfully generated transcript whose automatic insertion was cancelled or unavailable for the lifetime of the running application. The user SHALL be able to copy a pending transcript or dismiss it from the menu-bar or system-tray UI.

#### Scenario: User copies a pending transcript
- **WHEN** the user chooses to copy a pending transcript
- **THEN** the application SHALL place that transcript on the system clipboard and communicate that it is ready to paste

#### Scenario: User dismisses a pending transcript
- **WHEN** the user dismisses a pending transcript
- **THEN** the application SHALL remove it from the pending list without inserting it anywhere

#### Scenario: Application exits with pending transcripts
- **WHEN** the application exits while pending transcripts exist
- **THEN** the application SHALL make no claim that those in-memory transcripts will survive the exit

### Requirement: The application SHALL report platform capabilities and protect user data

The application SHALL communicate the availability of microphone access, global shortcut support, automatic text insertion, and credential storage for the current operating system. Focus inspection SHALL NOT be required for recording or automatic insertion. The application SHALL store provider credentials using the platform's secure credential storage and SHALL not retain captured audio after the transcription request completes or fails unless the user explicitly enables diagnostic retention.

#### Scenario: Platform does not support automatic insertion
- **WHEN** the current desktop environment permits recording and transcription but does not permit global text insertion
- **THEN** the application SHALL still allow transcription and SHALL present the result as pending text with a copy action

#### Scenario: Provider credentials are saved
- **WHEN** the user saves provider credentials
- **THEN** the application SHALL store them in platform-secure storage and SHALL not display the full credential in normal status or settings views

#### Scenario: Audio processing ends
- **WHEN** transcription succeeds or fails
- **THEN** the application SHALL release the captured audio and SHALL not retain a recording as part of normal operation
