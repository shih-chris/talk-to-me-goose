#![allow(unexpected_cfgs)]

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use arboard::Clipboard;
use core_graphics::{
    event::{CGEvent, CGEventFlags, CGEventTapLocation, KeyCode},
    event_source::{CGEventSource, CGEventSourceStateID},
};
use cpal::{
    Data, SampleFormat, Stream, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};
use macos_accessibility_client::accessibility::application_is_trusted;

use crate::{
    audio::AudioData,
    error::{AppError, AppResult},
    platform::{AudioRecorder, Capability, CredentialStore, HotkeyBackend, TextInserter},
};

pub struct MacHotkey {
    manager: Option<GlobalHotKeyManager>,
    hotkey: Option<HotKey>,
    error: Option<String>,
}

impl MacHotkey {
    pub fn new() -> Self {
        match GlobalHotKeyManager::new() {
            Ok(manager) => Self {
                manager: Some(manager),
                hotkey: None,
                error: None,
            },
            Err(error) => Self {
                manager: None,
                hotkey: None,
                error: Some(error.to_string()),
            },
        }
    }
}

impl HotkeyBackend for MacHotkey {
    fn register(&mut self, shortcut: &str) -> AppResult<()> {
        let normalized = shortcut
            .replace(' ', "")
            .replace("Command", "Super")
            .replace("Cmd", "Super");
        let hotkey: HotKey = normalized
            .parse()
            .map_err(|error| AppError::Config(format!("invalid shortcut: {error}")))?;
        let manager = self.manager.as_ref().ok_or_else(|| {
            AppError::Capability(
                self.error
                    .clone()
                    .unwrap_or_else(|| "global hotkey manager unavailable".to_string()),
            )
        })?;
        manager.register(hotkey).map_err(|error| {
            AppError::Capability(format!("could not register shortcut: {error}"))
        })?;
        self.hotkey = Some(hotkey);
        Ok(())
    }

    fn triggered(&mut self) -> bool {
        let Some(hotkey) = self.hotkey else {
            return false;
        };
        let receiver = GlobalHotKeyEvent::receiver();
        while let Ok(event) = receiver.try_recv() {
            if event.id() == hotkey.id() && event.state() == HotKeyState::Pressed {
                return true;
            }
        }
        false
    }

    fn capability(&self) -> Capability {
        match &self.error {
            Some(error) => Capability::unavailable(format!("global shortcut unavailable: {error}")),
            None => Capability::available("global shortcut manager initialized"),
        }
    }
}

pub struct MacAudioRecorder {
    stream: Option<Stream>,
    samples: Arc<Mutex<Vec<f32>>>,
    stream_error: Arc<Mutex<Option<String>>>,
    sample_rate: u32,
    max_samples: usize,
}

impl MacAudioRecorder {
    pub fn new() -> Self {
        Self {
            stream: None,
            samples: Arc::new(Mutex::new(Vec::new())),
            stream_error: Arc::new(Mutex::new(None)),
            sample_rate: 0,
            max_samples: 0,
        }
    }
}

impl AudioRecorder for MacAudioRecorder {
    fn start(&mut self, max_seconds: u64) -> AppResult<()> {
        if self.stream.is_some() {
            return Err(AppError::Audio("recording is already active".to_string()));
        }
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| AppError::Audio("no default microphone is available".to_string()))?;
        let supported = device
            .default_input_config()
            .map_err(|error| AppError::Audio(format!("could not inspect microphone: {error}")))?;
        let sample_rate = supported.sample_rate();
        let channels = supported.channels() as usize;
        let max_samples = (sample_rate as u64)
            .saturating_mul(max_seconds)
            .min(usize::MAX as u64) as usize;
        self.sample_rate = sample_rate;
        self.max_samples = max_samples;
        self.samples
            .lock()
            .map_err(|_| AppError::Audio("audio buffer is unavailable".to_string()))?
            .clear();
        *self
            .stream_error
            .lock()
            .map_err(|_| AppError::Audio("audio state is unavailable".to_string()))? = None;

        let config: StreamConfig = supported.clone().into();
        let samples = Arc::clone(&self.samples);
        let stream_error = Arc::clone(&self.stream_error);
        let max_samples_for_callback = self.max_samples;
        let stream = device
            .build_input_stream_raw(
                config,
                supported.sample_format(),
                move |data, _| {
                    let recorder = CallbackBuffer {
                        samples: &samples,
                        max_samples: max_samples_for_callback,
                    };
                    recorder.append(data, channels);
                },
                move |error| {
                    if let Ok(mut state) = stream_error.lock() {
                        *state = Some(error.to_string());
                    }
                },
                Some(Duration::from_secs(5)),
            )
            .map_err(|error| AppError::Audio(format!("could not start microphone: {error}")))?;
        stream.play().map_err(|error| {
            AppError::Audio(format!("could not play microphone stream: {error}"))
        })?;
        self.stream = Some(stream);
        Ok(())
    }

    fn stop(&mut self) -> AppResult<AudioData> {
        let Some(stream) = self.stream.take() else {
            return Err(AppError::Audio("recording is not active".to_string()));
        };
        drop(stream);
        if let Some(error) = self
            .stream_error
            .lock()
            .map_err(|_| AppError::Audio("audio state is unavailable".to_string()))?
            .take()
        {
            return Err(AppError::Audio(error));
        }
        let samples = self
            .samples
            .lock()
            .map_err(|_| AppError::Audio("audio buffer is unavailable".to_string()))?
            .clone();
        Ok(AudioData::new(samples, self.sample_rate))
    }

    fn is_recording(&self) -> bool {
        self.stream.is_some()
    }

    fn capability(&self) -> Capability {
        let host = cpal::default_host();
        if host.default_input_device().is_some() {
            Capability::available("default microphone detected")
        } else {
            Capability::unavailable("no default microphone detected")
        }
    }
}

struct CallbackBuffer<'a> {
    samples: &'a Arc<Mutex<Vec<f32>>>,
    max_samples: usize,
}

impl CallbackBuffer<'_> {
    fn append(&self, data: &Data, channels: usize) {
        let Ok(mut samples) = self.samples.lock() else {
            return;
        };
        let remaining = self.max_samples.saturating_sub(samples.len());
        if remaining == 0 {
            return;
        }
        match data.sample_format() {
            SampleFormat::F32 => append_frames(
                &mut samples,
                data.as_slice::<f32>().unwrap_or_default(),
                channels,
                remaining,
                |value| *value,
            ),
            SampleFormat::I16 => append_frames(
                &mut samples,
                data.as_slice::<i16>().unwrap_or_default(),
                channels,
                remaining,
                |value| *value as f32 / i16::MAX as f32,
            ),
            SampleFormat::U16 => append_frames(
                &mut samples,
                data.as_slice::<u16>().unwrap_or_default(),
                channels,
                remaining,
                |value| (*value as f32 - 32_768.0) / 32_768.0,
            ),
            _ => {}
        }
    }
}

fn append_frames<T, F>(
    samples: &mut Vec<f32>,
    data: &[T],
    channels: usize,
    remaining: usize,
    convert: F,
) where
    F: Fn(&T) -> f32,
{
    for frame in data.chunks(channels).take(remaining) {
        let mean = frame.iter().map(&convert).sum::<f32>() / frame.len() as f32;
        samples.push(mean.clamp(-1.0, 1.0));
    }
}

pub struct MacTextInserter;

impl MacTextInserter {
    const V_KEY: u16 = 0x09;

    fn paste() -> AppResult<()> {
        let source =
            CGEventSource::new(CGEventSourceStateID::CombinedSessionState).map_err(|_| {
                AppError::Clipboard("could not create keyboard event source".to_string())
            })?;
        let command_down = CGEvent::new_keyboard_event(source.clone(), KeyCode::COMMAND, true)
            .map_err(|_| AppError::Clipboard("could not create Command key event".to_string()))?;
        command_down.set_flags(CGEventFlags::CGEventFlagCommand);
        command_down.post(CGEventTapLocation::HID);
        let paste_down = CGEvent::new_keyboard_event(source.clone(), Self::V_KEY, true)
            .map_err(|_| AppError::Clipboard("could not create paste key event".to_string()))?;
        paste_down.set_flags(CGEventFlags::CGEventFlagCommand);
        paste_down.post(CGEventTapLocation::HID);
        let paste_up = CGEvent::new_keyboard_event(source.clone(), Self::V_KEY, false)
            .map_err(|_| AppError::Clipboard("could not create paste key event".to_string()))?;
        paste_up.set_flags(CGEventFlags::CGEventFlagCommand);
        paste_up.post(CGEventTapLocation::HID);
        let command_up = CGEvent::new_keyboard_event(source, KeyCode::COMMAND, false)
            .map_err(|_| AppError::Clipboard("could not create Command key event".to_string()))?;
        command_up.post(CGEventTapLocation::HID);
        Ok(())
    }
}

impl TextInserter for MacTextInserter {
    fn insert(&self, text: &str) -> AppResult<()> {
        if !application_is_trusted() {
            return Err(AppError::Capability(
                "Accessibility permission is required for automatic insertion; grant access to Talk To Me Goose in System Settings > Privacy & Security > Accessibility".to_string(),
            ));
        }
        let mut clipboard =
            Clipboard::new().map_err(|error| AppError::Clipboard(error.to_string()))?;
        let previous = clipboard.get_text().ok();
        clipboard
            .set_text(text.to_string())
            .map_err(|error| AppError::Clipboard(error.to_string()))?;
        Self::paste()?;
        std::thread::sleep(Duration::from_millis(100));
        match previous {
            Some(previous) => clipboard
                .set_text(previous)
                .map_err(|error| AppError::Clipboard(error.to_string()))?,
            None => clipboard
                .clear()
                .map_err(|error| AppError::Clipboard(error.to_string()))?,
        }
        Ok(())
    }

    fn copy(&self, text: &str) -> AppResult<()> {
        let mut clipboard =
            Clipboard::new().map_err(|error| AppError::Clipboard(error.to_string()))?;
        clipboard
            .set_text(text.to_string())
            .map_err(|error| AppError::Clipboard(error.to_string()))
    }

    fn capability(&self) -> Capability {
        if application_is_trusted() {
            Capability::available("Accessibility permission granted")
        } else {
            Capability::unavailable("Accessibility permission is required for automatic insertion")
        }
    }
}

pub struct MacCredentials;

impl CredentialStore for MacCredentials {
    fn capability(&self) -> Capability {
        match keyring::Entry::store_status() {
            Ok(()) => Capability::available("macOS Keychain available"),
            Err(error) => Capability::unavailable(format!("macOS Keychain unavailable: {error}")),
        }
    }
}
