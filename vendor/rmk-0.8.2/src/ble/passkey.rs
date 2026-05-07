use core::sync::atomic::{AtomicBool, Ordering};

use embassy_futures::select::{Either, select};
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Instant, with_deadline};
use rmk_types::keycode::KeyCode;
use trouble_host::prelude::{DefaultPacketPool, GattConnection, GattConnectionEvent};

pub(crate) const PASSKEY_ENTRY_TIMEOUT_SECS: u64 = 120;

pub fn passkey_entry_enabled() -> bool {
    true
}

pub(crate) struct PasskeyInputState {
    pub(crate) deadline: Option<Instant>,
    cleanup: Option<PasskeyCleanupGuard>,
}

impl PasskeyInputState {
    pub(crate) const fn new() -> Self {
        Self {
            deadline: None,
            cleanup: None,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.deadline = None;
        drop(self.cleanup.take());
    }

    pub(crate) fn begin(&mut self) {
        self.clear();
        begin_passkey_entry_session();
        self.cleanup = Some(PasskeyCleanupGuard::new());
        self.deadline = Some(Instant::now() + Duration::from_secs(PASSKEY_ENTRY_TIMEOUT_SECS));
    }
}

pub(crate) async fn next_gatt_event<'a, 'b>(
    conn: &GattConnection<'a, 'b, DefaultPacketPool>,
    passkey_state: &mut PasskeyInputState,
) -> Option<GattConnectionEvent<'a, 'b, DefaultPacketPool>> {
    if let Some(deadline) = passkey_state.deadline {
        return match select(conn.next(), with_deadline(deadline, PASSKEY_RESPONSE.wait())).await {
            Either::First(event) => Some(event),
            Either::Second(Ok(Some(passkey))) => {
                passkey_state.clear();

                info!("[gatt] Passkey entered: submitting");
                if let Err(e) = conn.raw().pass_key_input(passkey) {
                    error!("[gatt] pass_key_input error: {:?}", e);
                }
                None
            }
            Either::Second(Ok(None)) => {
                passkey_state.clear();

                info!("[gatt] Passkey entry cancelled");
                if let Err(e) = conn.raw().pass_key_cancel() {
                    error!("[gatt] pass_key_cancel error: {:?}", e);
                }
                None
            }
            Either::Second(Err(_)) => {
                passkey_state.clear();

                warn!("[gatt] Passkey entry timeout");
                let _ = conn.raw().pass_key_cancel();
                None
            }
        };
    }

    Some(conn.next().await)
}

pub const PASSKEY_LENGTH: usize = 6;

pub static PASSKEY_ENTRY_MODE: AtomicBool = AtomicBool::new(false);
pub static PASSKEY_RESPONSE: Signal<crate::RawMutex, Option<u32>> = Signal::new();

pub fn begin_passkey_entry_session() {
    PASSKEY_RESPONSE.reset();
    PASSKEY_ENTRY_MODE.store(true, Ordering::Release);
}

pub fn end_passkey_entry_session() {
    PASSKEY_ENTRY_MODE.store(false, Ordering::Release);
}

pub struct PasskeyCleanupGuard;

impl PasskeyCleanupGuard {
    pub const fn new() -> Self {
        Self
    }
}

impl Drop for PasskeyCleanupGuard {
    fn drop(&mut self) {
        end_passkey_entry_session();
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum PasskeyAction {
    DigitAdded,
    Submitted(u32),
    Cancelled,
    Backspaced,
    BufferFull,
    Incomplete,
    Ignored,
}

pub struct PasskeyEntryState {
    digits: [u8; PASSKEY_LENGTH],
    count: usize,
    active: bool,
}

impl PasskeyEntryState {
    pub const fn new() -> Self {
        Self {
            digits: [0; PASSKEY_LENGTH],
            count: 0,
            active: false,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn activate(&mut self) {
        self.active = true;
        self.reset();
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn check_mode_transition(&mut self) {
        let passkey_active = PASSKEY_ENTRY_MODE.load(Ordering::Acquire);
        if passkey_active && !self.is_active() {
            self.activate();
        } else if !passkey_active && self.is_active() {
            self.deactivate();
        }
    }

    pub fn reset(&mut self) {
        self.digits = [0; PASSKEY_LENGTH];
        self.count = 0;
    }

    pub fn add_digit(&mut self, digit: u8) -> bool {
        if self.count < PASSKEY_LENGTH {
            self.digits[self.count] = digit;
            self.count += 1;
            true
        } else {
            false
        }
    }

    pub fn remove_digit(&mut self) -> bool {
        if self.count > 0 {
            self.count -= 1;
            self.digits[self.count] = 0;
            true
        } else {
            false
        }
    }

    pub fn is_complete(&self) -> bool {
        self.count == PASSKEY_LENGTH
    }

    pub fn to_passkey(&self) -> u32 {
        let mut result = 0;
        for digit in self.digits.iter().take(self.count) {
            result = result * 10 + *digit as u32;
        }
        result
    }

    pub fn handle_key(&mut self, key: KeyCode) -> PasskeyAction {
        if let Some(digit) = keycode_to_digit(key) {
            if self.add_digit(digit) {
                PasskeyAction::DigitAdded
            } else {
                PasskeyAction::BufferFull
            }
        } else if matches!(key, KeyCode::Enter | KeyCode::KpEnter) {
            if self.is_complete() {
                let passkey = self.to_passkey();
                self.reset();
                PasskeyAction::Submitted(passkey)
            } else {
                PasskeyAction::Incomplete
            }
        } else if matches!(key, KeyCode::Escape) {
            self.reset();
            PasskeyAction::Cancelled
        } else if matches!(key, KeyCode::Backspace) {
            if self.remove_digit() {
                PasskeyAction::Backspaced
            } else {
                PasskeyAction::Ignored
            }
        } else {
            PasskeyAction::Ignored
        }
    }
}

pub fn keycode_to_digit(key: KeyCode) -> Option<u8> {
    match key {
        KeyCode::Kc1 | KeyCode::Kp1 => Some(1),
        KeyCode::Kc2 | KeyCode::Kp2 => Some(2),
        KeyCode::Kc3 | KeyCode::Kp3 => Some(3),
        KeyCode::Kc4 | KeyCode::Kp4 => Some(4),
        KeyCode::Kc5 | KeyCode::Kp5 => Some(5),
        KeyCode::Kc6 | KeyCode::Kp6 => Some(6),
        KeyCode::Kc7 | KeyCode::Kp7 => Some(7),
        KeyCode::Kc8 | KeyCode::Kp8 => Some(8),
        KeyCode::Kc9 | KeyCode::Kp9 => Some(9),
        KeyCode::Kc0 | KeyCode::Kp0 => Some(0),
        _ => None,
    }
}
