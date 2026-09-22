use std::{
    env,
    io::{self, Read},
    process::Command,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Instant,
};

use global_hotkey::GlobalHotKeyEvent;
use tao::{
    event::{Event, StartCause},
    event_loop::{ControlFlow, EventLoop, EventLoopBuilder, EventLoopProxy},
};
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

use talk_to_me_goose::{
    audio::AudioData,
    config::{AppConfig, ProviderKind},
    error::{AppError, AppResult},
    platform::{AudioRecorder, CredentialStore, HotkeyBackend, TextInserter},
    provider::transcriber_for,
    state::{SessionController, SessionPhase},
};
use tracing::{debug, error, info, warn};

mod logging;

#[cfg(target_os = "macos")]
use talk_to_me_goose::platform::macos::{
    MacAudioRecorder, MacCredentials, MacHotkey, MacTextInserter,
};
#[cfg(not(target_os = "macos"))]
use talk_to_me_goose::platform::unsupported::{
    ClipboardInserter, UnsupportedAudio, UnsupportedCredentials, UnsupportedHotkey,
};

const STATUS_ID: &str = "status";
const COPY_LATEST_ID: &str = "copy-latest";
const DISMISS_LATEST_ID: &str = "dismiss-latest";
const OPEN_CONFIG_ID: &str = "open-config";
const CONFIGURE_KEY_ID: &str = "configure-key";
const OPENAI_ID: &str = "provider-openai";
const GEMINI_ID: &str = "provider-gemini";
const QUIT_ID: &str = "quit";

#[derive(Debug)]
enum AppEvent {
    Wake,
    Menu(MenuEvent),
    Hotkey(GlobalHotKeyEvent),
    TranscriptionComplete(AppResult<String>),
}

struct MenuUi {
    menu: Menu,
    status: MenuItem,
    provider: MenuItem,
    capabilities: MenuItem,
    copy_latest: MenuItem,
    dismiss_latest: MenuItem,
    open_config: MenuItem,
    configure_key: MenuItem,
}

impl MenuUi {
    fn new(config: &AppConfig) -> AppResult<Self> {
        let menu = Menu::new();
        let status = MenuItem::with_id(STATUS_ID, "Idle", false, None);
        let provider = MenuItem::with_id(
            "provider",
            format!("Provider: {} / {}", config.provider.label(), config.model),
            false,
            None,
        );
        let capabilities =
            MenuItem::with_id("capabilities", "Capabilities: checking...", false, None);
        let copy_latest = MenuItem::with_id(
            COPY_LATEST_ID,
            "Copy latest pending transcript",
            false,
            None,
        );
        let dismiss_latest = MenuItem::with_id(
            DISMISS_LATEST_ID,
            "Dismiss latest pending transcript",
            false,
            None,
        );
        let open_config = MenuItem::with_id(OPEN_CONFIG_ID, "Open settings file", true, None);
        let configure_key = MenuItem::with_id(CONFIGURE_KEY_ID, "Configure API key...", true, None);
        let openai = MenuItem::with_id(OPENAI_ID, "Use OpenAI", true, None);
        let gemini = MenuItem::with_id(GEMINI_ID, "Use Gemini", true, None);
        let quit = MenuItem::with_id(QUIT_ID, "Quit", true, None);
        menu.append(&status)
            .and_then(|_| menu.append(&provider))
            .and_then(|_| menu.append(&capabilities))
            .and_then(|_| menu.append(&copy_latest))
            .and_then(|_| menu.append(&dismiss_latest))
            .and_then(|_| menu.append(&PredefinedMenuItem::separator()))
            .and_then(|_| menu.append(&open_config))
            .and_then(|_| menu.append(&configure_key))
            .and_then(|_| menu.append(&openai))
            .and_then(|_| menu.append(&gemini))
            .and_then(|_| menu.append(&PredefinedMenuItem::separator()))
            .and_then(|_| menu.append(&quit))
            .map_err(|error| AppError::Capability(format!("could not build tray menu: {error}")))?;
        Ok(Self {
            menu,
            status,
            provider,
            capabilities,
            copy_latest,
            dismiss_latest,
            open_config,
            configure_key,
        })
    }

    fn update(
        &self,
        config: &AppConfig,
        phase: SessionPhase,
        pending_count: usize,
        capabilities: &str,
        message: Option<&str>,
    ) {
        let status = match (phase, message) {
            (_, Some(message)) => format!("Status: {}", truncate(message, 80)),
            (SessionPhase::Idle, None) => "Status: Idle".to_string(),
            (SessionPhase::Recording, None) => "Status: Recording...".to_string(),
            (SessionPhase::Transcribing, None) => "Status: Transcribing...".to_string(),
        };
        self.status.set_text(status);
        self.provider.set_text(format!(
            "Provider: {} / {}",
            config.provider.label(),
            config.model
        ));
        self.capabilities
            .set_text(format!("Capabilities: {capabilities}"));
        self.copy_latest.set_enabled(pending_count > 0);
        self.dismiss_latest.set_enabled(pending_count > 0);
        self.open_config.set_enabled(true);
        self.configure_key.set_enabled(true);
    }
}

struct App {
    config: AppConfig,
    session: SessionController,
    recorder: Box<dyn AudioRecorder>,
    inserter: Box<dyn TextInserter>,
    credentials: Box<dyn CredentialStore>,
    hotkey: Box<dyn HotkeyBackend>,
    menu: MenuUi,
    event_proxy: EventLoopProxy<AppEvent>,
    transcription_rx: Receiver<AppEvent>,
    transcription_tx: Sender<AppEvent>,
    _tray: Option<TrayIcon>,
    message: Option<String>,
}

impl App {
    fn new(event_proxy: EventLoopProxy<AppEvent>) -> AppResult<Self> {
        let config = match AppConfig::load() {
            Ok(config) => {
                info!(
                    provider = %config.provider.label(),
                    model = %config.model,
                    shortcut = %config.shortcut,
                    max_recording_seconds = config.max_recording_seconds,
                    "configuration loaded"
                );
                config
            }
            Err(error) => {
                error!(error = %logging::safe_error(&error), "configuration load failed");
                return Err(error);
            }
        };
        if let Err(error) = config.save() {
            warn!(error = %logging::safe_error(&error), "configuration could not be saved");
        }
        let menu = MenuUi::new(&config)?;
        #[cfg(target_os = "macos")]
        let (recorder, inserter, hotkey, credentials): (
            Box<dyn AudioRecorder>,
            Box<dyn TextInserter>,
            Box<dyn HotkeyBackend>,
            Box<dyn CredentialStore>,
        ) = (
            Box::new(MacAudioRecorder::new()),
            Box::new(MacTextInserter),
            Box::new(MacHotkey::new()),
            Box::new(MacCredentials),
        );
        #[cfg(not(target_os = "macos"))]
        let (recorder, inserter, hotkey, credentials): (
            Box<dyn AudioRecorder>,
            Box<dyn TextInserter>,
            Box<dyn HotkeyBackend>,
            Box<dyn CredentialStore>,
        ) = (
            Box::new(UnsupportedAudio),
            Box::new(ClipboardInserter),
            Box::new(UnsupportedHotkey),
            Box::new(UnsupportedCredentials),
        );
        let (transcription_tx, transcription_rx) = mpsc::channel();
        let insertion_capability = inserter.capability();
        log_capability("global_shortcut", &hotkey.capability());
        log_capability("microphone", &recorder.capability());
        log_capability("automatic_insertion", &insertion_capability);
        log_capability("credential_store", &credentials.capability());
        Ok(Self {
            config,
            session: SessionController::new(),
            recorder,
            inserter,
            credentials,
            hotkey,
            menu,
            event_proxy,
            transcription_rx,
            transcription_tx,
            _tray: None,
            message: (!insertion_capability.is_available()).then(|| insertion_capability.detail),
        })
    }

    fn register_hotkey(&mut self) {
        info!(shortcut = %self.config.shortcut, "registering global shortcut");
        if let Err(error) = self.hotkey.register(&self.config.shortcut) {
            error!(error = %logging::safe_error(&error), "global shortcut registration failed");
            self.message = Some(error.to_string());
        } else {
            info!(shortcut = %self.config.shortcut, "global shortcut registered");
        }
    }

    fn handle_hotkey(&mut self) {
        info!(phase = ?self.session.phase(), "handling global shortcut");
        match self.session.phase() {
            SessionPhase::Idle => self.start_recording(),
            SessionPhase::Recording => self.stop_recording(),
            SessionPhase::Transcribing => {
                self.message = Some("Still transcribing the previous recording".to_string());
            }
        }
    }

    fn start_recording(&mut self) {
        if let Err(error) = self.recorder.start(self.config.max_recording_seconds) {
            warn!(capability = "microphone", error = %logging::safe_error(&error), "recording start failed");
            self.message = Some(error.to_string());
            return;
        }
        info!(
            max_recording_seconds = self.config.max_recording_seconds,
            "recording started"
        );
        if !self.session.start_recording() {
            let _ = self.recorder.stop();
            warn!("recording state rejected start");
            self.message = Some("A recording is already active".to_string());
            return;
        }
        self.message = None;
    }

    fn stop_recording(&mut self) {
        if !self.session.stop_recording() {
            warn!("recording stop requested outside recording state");
            return;
        }
        let audio = match self.recorder.stop() {
            Ok(audio) if !audio.is_empty() => {
                info!(
                    sample_count = audio.samples.len(),
                    duration_ms = (audio.duration_seconds() * 1000.0) as u64,
                    "recording stopped"
                );
                audio
            }
            Ok(_) => {
                self.session.transcription_failed();
                warn!("recording stopped without captured audio");
                self.message = Some("No audio was captured".to_string());
                return;
            }
            Err(error) => {
                self.session.transcription_failed();
                warn!(error = %logging::safe_error(&error), "recording stop failed");
                self.message = Some(error.to_string());
                return;
            }
        };
        self.spawn_transcription(audio);
    }

    fn spawn_transcription(&self, audio: AudioData) {
        let config = self.config.clone();
        let provider_label = config.provider.label().to_string();
        let sender = self.transcription_tx.clone();
        let proxy = self.event_proxy.clone();
        info!(
            provider = %config.provider.label(),
            model = %config.model,
            duration_ms = (audio.duration_seconds() * 1000.0) as u64,
            "transcription started"
        );
        thread::spawn(move || {
            let started = Instant::now();
            let request_config = config.clone();
            let result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    AppError::Provider(format!("could not start async runtime: {error}"))
                })
                .and_then(|runtime| {
                    runtime.block_on(async move {
                        let api_key = request_config.api_key()?.ok_or_else(|| {
                            warn!(provider = %request_config.provider.label(), category = "missing_api_key", "transcription could not start");
                            AppError::Provider(format!(
                                "no API key configured for {}",
                                request_config.provider.label()
                            ))
                        })?;
                        let transcriber = transcriber_for(&request_config)?;
                        transcriber
                            .transcribe(&audio, &request_config.model, &api_key)
                            .await
                    })
                });
            match &result {
                Ok(_) => info!(
                    provider = %provider_label,
                    elapsed_ms = started.elapsed().as_millis() as u64,
                    "transcription completed"
                ),
                Err(error) => warn!(
                    provider = %provider_label,
                    elapsed_ms = started.elapsed().as_millis() as u64,
                    error = %logging::safe_error(error),
                    "transcription failed"
                ),
            }
            let _ = sender.send(AppEvent::TranscriptionComplete(result));
            let _ = proxy.send_event(AppEvent::Wake);
        });
    }

    fn handle_transcription(&mut self, result: AppResult<String>) {
        match result {
            Ok(text) => {
                self.session.finish_transcription();
                if let Err(error) = self.inserter.insert(&text) {
                    self.session.retain_pending(text, error.to_string());
                    warn!(error = %logging::safe_error(&error), "transcript insertion failed");
                    self.message = Some(error.to_string());
                } else {
                    info!("transcript inserted");
                    self.message = Some("Inserted transcript".to_string());
                }
            }
            Err(error) => {
                self.session.transcription_failed();
                warn!(error = %logging::safe_error(&error), "transcription result unavailable");
                self.message = Some(error.to_string());
            }
        }
    }

    fn handle_menu_event(&mut self, id: &str) -> bool {
        info!(action = id, "menu action selected");
        match id {
            COPY_LATEST_ID => self.copy_latest(),
            DISMISS_LATEST_ID => self.dismiss_latest(),
            OPEN_CONFIG_ID => self.open_config(),
            CONFIGURE_KEY_ID => self.configure_key(),
            OPENAI_ID => self.select_provider(ProviderKind::OpenAi),
            GEMINI_ID => self.select_provider(ProviderKind::Gemini),
            QUIT_ID => return true,
            _ => {}
        }
        false
    }

    fn copy_latest(&mut self) {
        let Some(item) = self.session.pending().last() else {
            return;
        };
        let id = item.id;
        let text = item.text.clone();
        match self.inserter.copy(&text) {
            Ok(()) => {
                self.session.dismiss_pending(id);
                info!(pending_id = id, "pending transcript copied");
                self.message = Some("Copied pending transcript".to_string());
            }
            Err(error) => {
                warn!(error = %logging::safe_error(&error), "pending transcript copy failed");
                self.message = Some(error.to_string())
            }
        }
    }

    fn dismiss_latest(&mut self) {
        let id = self.session.pending().last().map(|item| item.id);
        if let Some(id) = id {
            self.session.dismiss_pending(id);
            info!(pending_id = id, "pending transcript dismissed");
            self.message = Some("Dismissed pending transcript".to_string());
        }
    }

    fn select_provider(&mut self, provider: ProviderKind) {
        info!(provider = %provider.label(), "provider selection changed");
        self.config.provider = provider;
        self.config.model = match provider {
            ProviderKind::OpenAi => "gpt-4o-mini-transcribe".to_string(),
            ProviderKind::Gemini => "gemini-2.5-flash".to_string(),
        };
        match self.config.save() {
            Ok(()) => {
                info!(provider = %provider.label(), "provider configuration saved");
                self.message = Some(format!("Using {}", provider.label()))
            }
            Err(error) => {
                warn!(error = %logging::safe_error(&error), "provider configuration save failed");
                self.message = Some(error.to_string())
            }
        }
    }

    fn open_config(&mut self) {
        match AppConfig::path() {
            Ok(path) => {
                let result = Command::new("open").arg(path).status();
                match result {
                    Ok(status) if status.success() => {
                        info!("settings file opened");
                        self.message = Some("Opened settings file".to_string());
                    }
                    Ok(status) => {
                        warn!(status = ?status.code(), "settings file open command failed");
                        self.message = Some("Could not open settings file".to_string());
                    }
                    Err(error) => {
                        warn!(error = %logging::sanitize(&error.to_string()), "settings file open command failed");
                        self.message = Some(AppError::Io(error).to_string());
                    }
                }
            }
            Err(error) => {
                warn!(error = %logging::safe_error(&error), "settings path unavailable");
                self.message = Some(error.to_string())
            }
        }
    }

    fn configure_key(&mut self) {
        #[cfg(target_os = "macos")]
        {
            info!(provider = %self.config.provider.label(), "opening API key dialog");
            let prompt = format!(
                "display dialog \"API key for {}\" default answer \"\" with hidden answer",
                self.config.provider.label()
            );
            let result = Command::new("osascript")
                .args(["-e", &prompt, "-e", "text returned of result"])
                .output();
            match result {
                Ok(output) if output.status.success() => {
                    let key = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    self.message = match self.config.set_api_key(&key) {
                        Ok(()) => {
                            info!(provider = %self.config.provider.label(), "API key saved");
                            Some("API key saved in Keychain".to_string())
                        }
                        Err(error) => {
                            warn!(error = %logging::safe_error(&error), "API key save failed");
                            Some(error.to_string())
                        }
                    };
                }
                Ok(_) => {
                    info!("API key dialog cancelled");
                    self.message = Some("API key dialog cancelled".to_string())
                }
                Err(error) => {
                    warn!(error = %logging::sanitize(&error.to_string()), "API key dialog failed");
                    self.message = Some(error.to_string())
                }
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            info!("API key configuration is unavailable from this platform UI");
            self.message = Some(
                "Configure the provider API key using the platform credential store".to_string(),
            );
        }
    }

    fn update_menu(&self) {
        let capabilities = format!(
            "shortcut {}, mic {}, insert {}, credentials {}",
            capability_label(self.hotkey.capability().is_available()),
            capability_label(self.recorder.capability().is_available()),
            capability_label(self.inserter.capability().is_available()),
            capability_label(self.credentials.capability().is_available()),
        );
        self.menu.update(
            &self.config,
            self.session.phase(),
            self.session.pending().count(),
            &capabilities,
            self.message.as_deref(),
        );
    }
}

fn create_icon() -> AppResult<Icon> {
    let size = 16;
    let mut rgba = Vec::with_capacity(size * size * 4);
    for y in 0..size {
        for x in 0..size {
            let border = x == 0 || x == size - 1 || y == 0 || y == size - 1;
            let eye = (x == 5 || x == 10) && (y == 5 || y == 6);
            let value = if border || eye { 255 } else { 80 };
            rgba.extend_from_slice(&[value, value, value, 255]);
        }
    }
    Icon::from_rgba(rgba, size as u32, size as u32)
        .map_err(|error| AppError::Capability(format!("could not create tray icon: {error}")))
}

fn run_set_api_key(provider: ProviderKind) -> AppResult<()> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let config = AppConfig {
        provider,
        ..AppConfig::default()
    };
    config.set_api_key(input.trim())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() == 3 && args[1] == "--set-api-key" {
        let provider = match args[2].as_str() {
            "openai" => ProviderKind::OpenAi,
            "gemini" => ProviderKind::Gemini,
            value => return Err(format!("unknown provider: {value}").into()),
        };
        run_set_api_key(provider)?;
        return Ok(());
    }

    let logging_state = logging::init();
    info!(
        version = env!("CARGO_PKG_VERSION"),
        process_id = std::process::id(),
        log_path = ?logging_state.path,
        fallback = logging_state.fallback,
        filter = %logging::filter_description(),
        "application starting"
    );
    if let Some(error) = &logging_state.error {
        warn!(error = %logging::sanitize(error), "file logging unavailable; using fallback");
    }

    let mut builder = EventLoopBuilder::<AppEvent>::with_user_event();
    let event_loop: EventLoop<AppEvent> = builder.build();
    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some({
        let proxy = proxy.clone();
        move |event: MenuEvent| {
            debug!(action = %event.id().as_ref(), "menu event received");
            let _ = proxy.send_event(AppEvent::Menu(event));
        }
    }));
    GlobalHotKeyEvent::set_event_handler(Some({
        let proxy = proxy.clone();
        move |event: GlobalHotKeyEvent| {
            debug!(id = event.id(), state = ?event.state(), "global hotkey event received");
            let _ = proxy.send_event(AppEvent::Hotkey(event));
        }
    }));

    let mut app = App::new(proxy.clone())?;
    app.register_hotkey();
    app.update_menu();
    let icon = create_icon()?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::NewEvents(StartCause::Init) => {
                app._tray = TrayIconBuilder::new()
                    .with_menu(Box::new(app.menu.menu.clone()))
                    .with_tooltip("Talk To Me Goose")
                    .with_icon(icon.clone())
                    .build()
                    .ok();
            }
            Event::UserEvent(AppEvent::TranscriptionComplete(result)) => {
                app.handle_transcription(result);
                app.update_menu();
            }
            Event::UserEvent(AppEvent::Menu(event)) => {
                if app.handle_menu_event(event.id().as_ref()) {
                    info!("application shutdown requested");
                    *control_flow = ControlFlow::Exit;
                }
                app.update_menu();
            }
            Event::UserEvent(AppEvent::Hotkey(event)) => {
                info!(id = event.id(), state = ?event.state(), "global hotkey event dispatched");
                if event.state() == global_hotkey::HotKeyState::Pressed {
                    app.handle_hotkey();
                    app.update_menu();
                }
            }
            Event::UserEvent(AppEvent::Wake) | Event::MainEventsCleared => {
                while let Ok(event) = app.transcription_rx.try_recv() {
                    if let AppEvent::TranscriptionComplete(result) = event {
                        app.handle_transcription(result);
                    }
                }
                app.update_menu();
            }
            _ => {}
        }
    });
}

fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn log_capability(name: &str, capability: &talk_to_me_goose::platform::Capability) {
    if capability.is_available() {
        info!(capability = name, detail = %logging::sanitize(&capability.detail), "platform capability available");
    } else {
        warn!(capability = name, detail = %logging::sanitize(&capability.detail), "platform capability unavailable");
    }
}

fn capability_label(available: bool) -> &'static str {
    if available { "ok" } else { "unavailable" }
}
