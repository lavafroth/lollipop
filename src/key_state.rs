use std::fmt::Debug;
use std::time::Duration;
use std::time::SystemTime;

#[derive(Clone, Copy, PartialEq)]
pub enum KeyState {
    Latched(SystemTime),
    Locked,
    None,
}

impl KeyState {
    pub(crate) fn transition(&mut self, time: SystemTime, timeout: Duration) {
        *self = match self {
            KeyState::Latched(last_press) => {
                if let Ok(elapsed) = time.duration_since(*last_press)
                    && elapsed < timeout
                {
                    KeyState::Locked
                } else {
                    KeyState::None
                }
            }
            KeyState::Locked => KeyState::None,
            KeyState::None => KeyState::Latched(time),
        }
    }

    pub(crate) fn pressed_state(&self) -> i32 {
        match self {
            KeyState::Locked | KeyState::Latched(_) => 1,
            KeyState::None => 0,
        }
    }
}

impl Debug for KeyState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Latched(time) => write!(
                f,
                "Latched {}s",
                time.elapsed().unwrap_or_default().as_secs()
            ),
            Self::Locked => write!(f, "Locked"),
            Self::None => write!(f, "None"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const START_TIME: SystemTime = SystemTime::UNIX_EPOCH;

    #[test]
    fn test_initial_state() {
        let state = KeyState::None;
        assert_eq!(state.pressed_state(), 0);
    }

    #[test]
    fn test_none_to_latched_transition() {
        let mut state = KeyState::None;

        state.transition(START_TIME, Duration::from_secs(1));

        assert!(matches!(state, KeyState::Latched(_)));
        assert_eq!(state.pressed_state(), 1);
    }

    #[test]
    fn test_latched_to_locked_transition() {
        let timeout = Duration::from_secs(1);
        let mut state = KeyState::Latched(START_TIME);

        let double_tap_at = START_TIME + Duration::from_millis(500);
        state.transition(double_tap_at, timeout); // quick double tap

        assert_eq!(state, KeyState::Locked);
        assert_eq!(state.pressed_state(), 1);
    }

    #[test]
    fn test_locked_to_none_transition() {
        let timeout = Duration::from_secs(1);
        let mut state = KeyState::Locked;

        state.transition(START_TIME + Duration::from_secs(2), timeout); // unlock

        assert_eq!(state, KeyState::None);
        assert_eq!(state.pressed_state(), 0);
    }

    // timeout cases

    #[test]
    fn test_latched_timeout_failure() {
        let timeout = Duration::from_millis(200);

        let mut state = KeyState::Latched(START_TIME);
        let expired_at = START_TIME + Duration::from_millis(300);
        state.transition(expired_at, timeout); // second tap is too late

        assert_eq!(state, KeyState::None);
        assert_eq!(state.pressed_state(), 0);
    }

    #[test]
    fn test_boundary_condition_just_before_timeout() {
        let timeout = Duration::from_millis(100);
        let mut state = KeyState::Latched(START_TIME);

        let double_tap = START_TIME + Duration::from_millis(99);
        state.transition(double_tap, timeout);

        assert_eq!(state, KeyState::Locked);
        assert_eq!(state.pressed_state(), 1);
    }

    #[test]
    fn test_boundary_condition_at_timeout() {
        let timeout = Duration::from_millis(100);
        let mut state = KeyState::Latched(START_TIME);

        let expired_at = START_TIME + Duration::from_millis(100);
        state.transition(expired_at, timeout);

        assert_eq!(state, KeyState::None);
        assert_eq!(state.pressed_state(), 0);
    }

    // full lifecycles

    #[test]
    fn test_full_lifecycle_success() {
        let timeout = Duration::from_secs(5);
        let mut state = KeyState::None;

        state.transition(START_TIME, timeout); // latch
        assert!(matches!(state, KeyState::Latched(_)));

        let t1 = START_TIME + Duration::from_millis(100);
        state.transition(t1, timeout); // lock
        assert_eq!(state, KeyState::Locked);

        let t2 = t1 + Duration::from_secs(1);
        state.transition(t2, timeout); // unlock
        assert_eq!(state, KeyState::None);

        let t3 = t2 + Duration::from_secs(1);
        state.transition(t3, timeout); // latch
        assert!(matches!(state, KeyState::Latched(_)));
    }

    #[test]
    fn test_release_after_failure() {
        let timeout = Duration::from_millis(200);
        let mut state = KeyState::None;
        state.transition(START_TIME, timeout); // latch

        let expired_at = START_TIME + Duration::from_millis(300);
        state.transition(expired_at, timeout); // expired second tap
        assert_eq!(state, KeyState::None);

        let t_repress = START_TIME + Duration::from_secs(1);
        state.transition(t_repress, timeout); // latch again
        assert!(matches!(state, KeyState::Latched(_)));
    }
}
