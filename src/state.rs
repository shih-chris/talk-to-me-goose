use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionPhase {
    Idle,
    Recording,
    Transcribing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingTranscript {
    pub id: u64,
    pub text: String,
    pub reason: String,
}

#[derive(Debug)]
pub struct SessionController {
    phase: SessionPhase,
    pending: VecDeque<PendingTranscript>,
    next_pending_id: u64,
}

impl Default for SessionController {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionController {
    pub fn new() -> Self {
        Self {
            phase: SessionPhase::Idle,
            pending: VecDeque::new(),
            next_pending_id: 1,
        }
    }

    pub fn phase(&self) -> SessionPhase {
        self.phase
    }

    pub fn start_recording(&mut self) -> bool {
        if self.phase != SessionPhase::Idle {
            return false;
        }
        self.phase = SessionPhase::Recording;
        true
    }

    pub fn stop_recording(&mut self) -> bool {
        if self.phase != SessionPhase::Recording {
            return false;
        }
        self.phase = SessionPhase::Transcribing;
        true
    }

    pub fn transcription_failed(&mut self) {
        self.phase = SessionPhase::Idle;
    }

    pub fn finish_transcription(&mut self) {
        self.phase = SessionPhase::Idle;
    }

    pub fn pending(&self) -> impl Iterator<Item = &PendingTranscript> {
        self.pending.iter()
    }

    pub fn pending_text(&self, id: u64) -> Option<&str> {
        self.pending
            .iter()
            .find(|item| item.id == id)
            .map(|item| item.text.as_str())
    }

    pub fn retain_pending(&mut self, text: String, reason: impl Into<String>) {
        let pending = PendingTranscript {
            id: self.next_pending_id,
            text,
            reason: reason.into(),
        };
        self.next_pending_id += 1;
        self.pending.push_back(pending);
    }

    pub fn take_pending(&mut self, id: u64) -> Option<PendingTranscript> {
        let index = self.pending.iter().position(|item| item.id == id)?;
        self.pending.remove(index)
    }

    pub fn dismiss_pending(&mut self, id: u64) -> bool {
        self.take_pending(id).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::{SessionController, SessionPhase};

    #[test]
    fn toggles_recording_and_transcription() {
        let mut session = SessionController::new();
        assert!(session.start_recording());
        assert_eq!(session.phase(), SessionPhase::Recording);
        assert!(session.stop_recording());
        assert_eq!(session.phase(), SessionPhase::Transcribing);
    }

    #[test]
    fn rejects_busy_recording() {
        let mut session = SessionController::new();
        assert!(session.start_recording());
        assert!(!session.start_recording());
        assert!(session.stop_recording());
        assert!(!session.start_recording());
    }

    #[test]
    fn transcription_does_not_depend_on_start_focus() {
        let mut session = SessionController::new();
        assert!(session.start_recording());
        assert!(session.stop_recording());

        session.finish_transcription();
        assert_eq!(session.pending().count(), 0);
    }

    #[test]
    fn transcription_failure_returns_to_idle() {
        let mut session = SessionController::new();
        assert!(session.start_recording());
        assert!(session.stop_recording());

        session.transcription_failed();
        assert_eq!(session.phase(), SessionPhase::Idle);
        assert_eq!(session.pending().count(), 0);
    }

    #[test]
    fn insertion_failure_retains_pending_transcript() {
        let mut session = SessionController::new();
        session.retain_pending("hello".to_string(), "automatic insertion unavailable");

        let pending = session.pending().next().expect("pending transcript");
        assert_eq!(pending.text, "hello");
        assert_eq!(session.pending_text(pending.id), Some("hello"));
    }

    #[test]
    fn pending_transcript_can_be_dismissed() {
        let mut session = SessionController::new();
        session.retain_pending("hello".to_string(), "insertion failed");
        let id = session.pending().next().expect("pending transcript").id;

        assert!(session.dismiss_pending(id));
        assert_eq!(session.pending().count(), 0);
        assert!(!session.dismiss_pending(id));
    }
}
