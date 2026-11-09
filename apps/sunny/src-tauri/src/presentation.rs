//! Cancellation tokens for background captures; closing a window must cancel its pending show.

use std::sync::Mutex;

#[derive(Default)]
struct RequestState {
    generation: u64,
    pending: bool,
}

#[derive(Default)]
pub struct ShowRequest(Mutex<RequestState>);

impl ShowRequest {
    pub fn begin(&self) -> Option<u64> {
        let mut state = self.0.lock().ok()?;
        if state.pending {
            return None;
        }
        state.generation = state.generation.wrapping_add(1);
        state.pending = true;
        Some(state.generation)
    }

    pub fn is_pending(&self) -> bool {
        self.0.lock().map(|state| state.pending).unwrap_or(false)
    }

    pub fn cancel(&self) {
        if let Ok(mut state) = self.0.lock() {
            state.generation = state.generation.wrapping_add(1);
            state.pending = false;
        }
    }

    pub fn finish(&self, generation: u64) -> bool {
        let Ok(mut state) = self.0.lock() else {
            return false;
        };
        if !state.pending || state.generation != generation {
            return false;
        }
        state.pending = false;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_prevents_a_late_capture_from_reopening_the_window() {
        let request = ShowRequest::default();
        let ticket = request.begin().expect("first show");
        assert!(request.is_pending());
        request.cancel();
        assert!(!request.finish(ticket));
        assert!(!request.is_pending());
    }

    #[test]
    fn overlapping_open_requests_are_coalesced() {
        let request = ShowRequest::default();
        let ticket = request.begin().expect("first show");
        assert!(request.begin().is_none());
        assert!(request.finish(ticket));
        assert!(!request.finish(ticket));
    }

    #[test]
    fn an_old_job_cannot_complete_a_new_request() {
        let request = ShowRequest::default();
        let old = request.begin().expect("first show");
        request.cancel();
        let fresh = request.begin().expect("next show");
        assert!(!request.finish(old));
        assert!(request.is_pending());
        assert!(request.finish(fresh));
    }
}
