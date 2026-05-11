use std::fmt::Display;

use tokio::time::Instant;

use std::time::Duration;

pub struct Touchpad {
    pub state: TouchState,
    pub position: [i32; 2],
    pub timeout: Duration,
    pub slop: u64,
}

#[derive(PartialEq, Eq)]
pub enum TouchState {
    Idle,
    Pending(Instant),
    DoubleTap,
    Swipe,
}

impl Display for TouchState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TouchState::Idle => write!(f, "Idle"),
            TouchState::Pending(_time) => write!(f, "Pending"),
            TouchState::DoubleTap => write!(f, "DoubleTap"),
            TouchState::Swipe => write!(f, "Swipe"),
        }
    }
}

impl Touchpad {
    pub async fn timeout(&self) {
        if let TouchState::Pending(time) = self.state {
            let deadline = time + self.timeout;
            tokio::time::sleep_until(deadline).await;
        } else {
            std::future::pending::<()>().await;
        }
    }
}

pub const TOUCH_RELEASED: i32 = 0;
pub const TOUCH_HELD: i32 = 1;
pub const COORDINATE_EMPTY: i32 = -1;
pub const POSITION_EMPTY: [i32; 2] = [-1, -1];

impl Touchpad {
    pub fn respond_touch(&mut self, touch: i32) {
        use TouchState::*;
        if touch == TOUCH_HELD
            && let Pending(_) = self.state
        {
            self.state = DoubleTap
        }

        if touch == TOUCH_RELEASED {
            self.state = match self.state {
                //  After a double tap, whether the cursor was being dragged or not, it's time to release the latched keys.
                Idle | Pending(_) | DoubleTap => Pending(Instant::now()),

                // the cursor was being moved, do nothing
                Swipe => Idle,
            };
            self.position = POSITION_EMPTY;
        }
        // eprint!("{} ", self.state);
    }

    /// Transitions idle state to swipe when dragged beyond square.
    pub fn respond_motion(&mut self, axis: usize, coordinate: i32) {
        if self.state != TouchState::Idle {
            return;
        }

        if self.position[axis] == COORDINATE_EMPTY {
            self.position[axis] = coordinate;
            return;
        }

        let cursor_dragged_beyond_threshold_square =
            (self.position[axis] - coordinate).abs() as u64 > self.slop;
        if cursor_dragged_beyond_threshold_square && self.state == TouchState::Idle {
            self.state = TouchState::Swipe;
        }
        // eprint!("{} ", self.state);
    }
}
