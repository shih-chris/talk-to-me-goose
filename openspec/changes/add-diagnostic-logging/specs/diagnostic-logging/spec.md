## Purpose

Provide privacy-safe, near-real-time visibility into the background application's lifecycle and platform integrations so users can diagnose why recording, shortcuts, permissions, transcription, or insertion did not work.

## ADDED Requirements

### Requirement: The application SHALL write discoverable diagnostic logs

The application SHALL initialize diagnostic logging during startup and SHALL write timestamped entries with severity to a local file in a documented, platform-appropriate user data or log directory. Log entries SHALL be flushed promptly enough to support following the file while the application is running.

#### Scenario: Application starts normally
- **WHEN** the application launches with a writable user data or log directory
- **THEN** it SHALL create or append to its diagnostic log and record startup context, including the log path and application version

#### Scenario: Log directory is unavailable
- **WHEN** the application cannot create or write its normal log file
- **THEN** it SHALL continue attempting to run and SHALL expose the logging failure through an available fallback such as standard error or the status UI

#### Scenario: User follows the log during runtime
- **WHEN** a user reads the documented log file while the application is running
- **THEN** newly emitted entries SHALL become visible without requiring application shutdown

### Requirement: The application SHALL log workflow and platform diagnostics

The application SHALL record useful non-sensitive events for startup, configuration loading, platform capability checks, menu actions, global shortcut registration and events, microphone recording, transcription lifecycle, current-destination text insertion, pending transcript recovery, and shutdown. Each failure entry SHALL identify the affected operation and include an actionable error description when available.

#### Scenario: Global shortcut is registered and activated
- **WHEN** the application attempts to register the configured shortcut or receives a shortcut event
- **THEN** the log SHALL record the registration result or event state without logging audio or transcript content

#### Scenario: Permission or capability check fails
- **WHEN** microphone access, accessibility access, automatic insertion, credential storage, or shortcut registration is unavailable
- **THEN** the log SHALL identify the unavailable capability and the reason reported by the platform or application

#### Scenario: Dictation session changes state
- **WHEN** a session starts recording, stops recording, begins transcription, completes, inserts, becomes pending, or fails
- **THEN** the log SHALL record the state transition and relevant non-sensitive metadata such as duration, provider, model, or error category

#### Scenario: Menu action is selected
- **WHEN** the user selects a menu action such as opening settings, configuring a key, changing providers, copying pending text, dismissing pending text, or quitting
- **THEN** the log SHALL record the action identifier and its success or failure without logging the action's secret or text payload

### Requirement: The application SHALL protect sensitive data in logs

The application MUST NOT write API keys, captured audio, transcript text, clipboard contents, or full secret-bearing provider request or response bodies to diagnostic logs. Errors SHALL be sanitized when their underlying message may contain request data or credentials.

#### Scenario: Provider request is logged
- **WHEN** a transcription request starts or finishes
- **THEN** the log SHALL identify the provider, model, timing, and outcome without including the API key, audio bytes, transcript, or complete request payload

#### Scenario: Clipboard or pending transcript action is logged
- **WHEN** text is copied, inserted, retained, or dismissed
- **THEN** the log SHALL record the operation and outcome without recording the text itself or clipboard contents

#### Scenario: Sensitive error is returned
- **WHEN** a provider or platform error may contain credentials, request bodies, or response payloads
- **THEN** the application SHALL log a sanitized error category and safe summary instead of the raw sensitive value

### Requirement: The application SHALL document log access

The application documentation SHALL identify the log path or path pattern, explain how to follow the log in real time, describe the information that may appear in it, and warn users not to share logs without reviewing them for local application names and error details.

#### Scenario: User needs to report a shortcut failure
- **WHEN** the user follows the troubleshooting documentation
- **THEN** they SHALL be able to locate and follow the current log while reproducing the failure

#### Scenario: User shares diagnostics
- **WHEN** the user prepares a diagnostic log for another person
- **THEN** the documentation SHALL tell them which non-secret contextual values may still require review before sharing
