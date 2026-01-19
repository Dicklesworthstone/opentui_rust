//! Keyboard event types.

use bitflags::bitflags;

bitflags! {
    /// Keyboard modifier flags.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct KeyModifiers: u8 {
        /// Shift key.
        const SHIFT = 0b0000_0001;
        /// Alt/Option key.
        const ALT = 0b0000_0010;
        /// Control key.
        const CTRL = 0b0000_0100;
        /// Super/Meta/Windows key (not widely supported).
        const SUPER = 0b0000_1000;
        /// Hyper key (rarely used).
        const HYPER = 0b0001_0000;
        /// Meta key (rarely used, distinct from Alt on some systems).
        const META = 0b0010_0000;
    }
}

/// A key code representing a keyboard key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyCode {
    /// Backspace key.
    Backspace,
    /// Enter/Return key.
    Enter,
    /// Left arrow key.
    Left,
    /// Right arrow key.
    Right,
    /// Up arrow key.
    Up,
    /// Down arrow key.
    Down,
    /// Home key.
    Home,
    /// End key.
    End,
    /// Page Up key.
    PageUp,
    /// Page Down key.
    PageDown,
    /// Tab key.
    Tab,
    /// Shift+Tab (backtab).
    BackTab,
    /// Delete key.
    Delete,
    /// Insert key.
    Insert,
    /// Function key (F1-F24).
    F(u8),
    /// A character key (includes space).
    Char(char),
    /// Escape key.
    Esc,
    /// Caps Lock key (rarely reported).
    CapsLock,
    /// Scroll Lock key.
    ScrollLock,
    /// Num Lock key.
    NumLock,
    /// Print Screen key.
    PrintScreen,
    /// Pause key.
    Pause,
    /// Menu key.
    Menu,
    /// Keypad Begin (numpad 5 without numlock).
    KeypadBegin,
    /// Null (Ctrl+Space or Ctrl+@).
    Null,
}

impl KeyCode {
    /// Check if this is a function key.
    #[must_use]
    pub fn is_function_key(&self) -> bool {
        matches!(self, Self::F(_))
    }

    /// Check if this is a character key.
    #[must_use]
    pub fn is_char(&self) -> bool {
        matches!(self, Self::Char(_))
    }

    /// Check if this is a navigation key (arrows, home, end, page up/down).
    #[must_use]
    pub fn is_navigation(&self) -> bool {
        matches!(
            self,
            Self::Left
                | Self::Right
                | Self::Up
                | Self::Down
                | Self::Home
                | Self::End
                | Self::PageUp
                | Self::PageDown
        )
    }

    /// Get the character if this is a character key.
    #[must_use]
    pub fn char(&self) -> Option<char> {
        match self {
            Self::Char(c) => Some(*c),
            _ => None,
        }
    }
}

/// A keyboard event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    /// The key code.
    pub code: KeyCode,
    /// Modifier keys held.
    pub modifiers: KeyModifiers,
}

impl KeyEvent {
    /// Create a new key event.
    #[must_use]
    pub fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { code, modifiers }
    }

    /// Create a key event with no modifiers.
    #[must_use]
    pub fn key(code: KeyCode) -> Self {
        Self::new(code, KeyModifiers::empty())
    }

    /// Create a character key event.
    #[must_use]
    pub fn char(c: char) -> Self {
        Self::key(KeyCode::Char(c))
    }

    /// Create a Ctrl+key event.
    #[must_use]
    pub fn with_ctrl(code: KeyCode) -> Self {
        Self::new(code, KeyModifiers::CTRL)
    }

    /// Create an Alt+key event.
    #[must_use]
    pub fn with_alt(code: KeyCode) -> Self {
        Self::new(code, KeyModifiers::ALT)
    }

    /// Check if Shift is held.
    #[must_use]
    pub fn shift(&self) -> bool {
        self.modifiers.contains(KeyModifiers::SHIFT)
    }

    /// Check if Ctrl is held.
    #[must_use]
    pub fn ctrl(&self) -> bool {
        self.modifiers.contains(KeyModifiers::CTRL)
    }

    /// Check if Alt is held.
    #[must_use]
    pub fn alt(&self) -> bool {
        self.modifiers.contains(KeyModifiers::ALT)
    }

    /// Check if this matches a specific key with optional modifiers.
    #[must_use]
    pub fn matches(&self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        self.code == code && self.modifiers == modifiers
    }

    /// Check if this is Ctrl+C.
    #[must_use]
    pub fn is_ctrl_c(&self) -> bool {
        self.matches(KeyCode::Char('c'), KeyModifiers::CTRL)
    }

    /// Check if this is Ctrl+D.
    #[must_use]
    pub fn is_ctrl_d(&self) -> bool {
        self.matches(KeyCode::Char('d'), KeyModifiers::CTRL)
    }

    /// Check if this is Escape.
    #[must_use]
    pub fn is_esc(&self) -> bool {
        self.code == KeyCode::Esc
    }

    /// Check if this is Enter.
    #[must_use]
    pub fn is_enter(&self) -> bool {
        self.code == KeyCode::Enter
    }
}

impl From<char> for KeyEvent {
    fn from(c: char) -> Self {
        Self::char(c)
    }
}

impl From<KeyCode> for KeyEvent {
    fn from(code: KeyCode) -> Self {
        Self::key(code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_event_creation() {
        let event = KeyEvent::char('a');
        assert_eq!(event.code, KeyCode::Char('a'));
        assert!(event.modifiers.is_empty());
    }

    #[test]
    fn test_key_event_modifiers() {
        let event = KeyEvent::with_ctrl(KeyCode::Char('c'));
        assert!(event.ctrl());
        assert!(!event.shift());
        assert!(!event.alt());
        assert!(event.is_ctrl_c());
    }

    #[test]
    fn test_key_code_checks() {
        assert!(KeyCode::F(1).is_function_key());
        assert!(KeyCode::Char('x').is_char());
        assert!(KeyCode::Up.is_navigation());
        assert!(!KeyCode::Enter.is_navigation());
    }

    #[test]
    fn test_key_event_from_char() {
        let event: KeyEvent = 'z'.into();
        assert_eq!(event.code, KeyCode::Char('z'));
    }
}
