//! lyra-core: pure domain logic, no I/O, no Qt, no async.
//! Unit-testable with plain `cargo test -p lyra-core`.

pub mod queue;
pub use queue::PlayQueue;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatMode {
    Off,
    All,
    One,
}

/// Next index in a play queue. `None` means "stop" (end of queue with
/// `Off`, or an empty queue). Pure: same inputs -> same output.
pub fn next_index(current: usize, len: usize, mode: RepeatMode) -> Option<usize> {
    if len == 0 {
        return None;
    }
    match mode {
        RepeatMode::One => Some(current),
        RepeatMode::All => Some((current + 1) % len),
        RepeatMode::Off => {
            let next = current + 1;
            if next < len { Some(next) } else { None }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advances_within_queue() {
        assert_eq!(next_index(0, 3, RepeatMode::Off), Some(1));
        assert_eq!(next_index(1, 3, RepeatMode::Off), Some(2));
    }

    #[test]
    fn stops_at_end_when_repeat_off() {
        assert_eq!(next_index(2, 3, RepeatMode::Off), None);
    }

    #[test]
    fn wraps_when_repeat_all() {
        assert_eq!(next_index(2, 3, RepeatMode::All), Some(0));
    }

    #[test]
    fn repeat_one_holds_position() {
        assert_eq!(next_index(1, 3, RepeatMode::One), Some(1));
    }

    #[test]
    fn empty_queue_yields_none() {
        assert_eq!(next_index(0, 0, RepeatMode::All), None);
    }
}
