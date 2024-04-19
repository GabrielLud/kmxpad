use interception as ic;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    Up,
    Down,
}

impl fmt::Display for KeyState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            KeyState::Up => write!(f, "Up"),
            KeyState::Down => write!(f, "Up")
        }
    }
}

impl From<ic::KeyState> for KeyState {
    fn from(key_state: ic::KeyState) -> Self {
        if key_state.contains(ic::KeyState::UP) {
            KeyState::Up
        } else {
            KeyState::Down
        }
    }
}

#[derive(Serialize, Deserialize, Hash, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Button4,
    Button5,
}

impl fmt::Display for MouseButton {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MouseButton::Left => write!(f, "Left"),
            MouseButton::Right => write!(f, "Right"),
            MouseButton::Middle => write!(f, "Middle"),
            MouseButton::Button4 => write!(f, "Button4"),
            MouseButton::Button5 => write!(f, "Button5"),
        }
    }
}

#[derive(Serialize, Deserialize, Hash, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ControllerButton {
    DpadUp = 1,
    DpadDown = 2,
    DpadLeft = 4,
    DpadRight = 8,

    Start = 16,
    Back = 32,

    LeftThumb = 64,
    RightThumb = 128,

    LeftShoulder = 256,
    RightShoulder = 512,

    Guide = 1024,

    A = 4096,
    B = 8192,
    X = 16384,
    Y = 32768,

    LeftTrigger,
    RightTrigger,
}

impl fmt::Display for ControllerButton {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ControllerButton::DpadUp => write!(f, "DpadUp"),
            ControllerButton::DpadDown => write!(f, "DpadDown"),
            ControllerButton::DpadLeft => write!(f, "DpadLeft"),
            ControllerButton::DpadRight => write!(f, "DpadRight"),
            ControllerButton::Start => write!(f, "Start"),
            ControllerButton::Back => write!(f, "Back"),
            ControllerButton::LeftThumb => write!(f, "LeftThumb"),
            ControllerButton::RightThumb => write!(f, "RightThumb"),
            ControllerButton::LeftShoulder => write!(f, "LeftShoulder"),
            ControllerButton::RightShoulder => write!(f, "RightShoulder"),
            ControllerButton::Guide => write!(f, "Guide"),
            ControllerButton::A => write!(f, "A"),
            ControllerButton::B => write!(f, "B"),
            ControllerButton::X => write!(f, "X"),
            ControllerButton::Y => write!(f, "Y"),
            ControllerButton::LeftTrigger => write!(f, "LeftTrigger"),
            ControllerButton::RightTrigger => write!(f, "RightTrigger"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    MouseMove(i32, i32),
    MouseButton(MouseButton, KeyState),
    Keyboard(ic::ScanCode, KeyState),
    Reset,
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Event::MouseMove(x, y) => write!(f, "MouseMove({}, {})", x, y),
            Event::MouseButton(button, state) => write!(f, "MouseButton({}, {:?})", button, state),
            Event::Keyboard(scan_code, state) => write!(f, "Keyboard({:?}, {:?})", scan_code, state),
            Event::Reset => write!(f, "Reset"),
        }
    }
}