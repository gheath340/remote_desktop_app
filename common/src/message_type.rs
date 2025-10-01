//enum for all different message types
#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MessageType {
    // Session
    Text        = 0x01,
    Connect     = 0x02,
    Disconnect  = 0x03,
    Error       = 0x04,

    // Display / Frames
    FrameFull   = 0x10,
    FrameDelta  = 0x11,
    CursorShape = 0x12,
    CursorPos   = 0x13,
    Resize      = 0x14,

    // Input
    KeyDown     = 0x20,
    KeyUp       = 0x21,
    MouseMove   = 0x22,
    MouseDown   = 0x23,
    MouseUp     = 0x24,
    MouseScroll = 0x25,

    // Clipboard
    Clipboard   = 0x30,

    //Catch all others
    Unknown(u8),
}

impl MessageType {
    //take in u8 and return the correct message type
    pub fn from_u8(v: u8) -> Self {
        match v {
            0x01 => MessageType::Text,
            0x02 => MessageType::Connect,
            0x03 => MessageType::Disconnect,
            0x04 => MessageType::Error,

            0x10 => MessageType::FrameFull,
            0x11 => MessageType::FrameDelta,
            0x12 => MessageType::CursorShape,
            0x13 => MessageType::CursorPos,
            0x14 => MessageType::Resize,

            0x20 => MessageType::KeyDown,
            0x21 => MessageType::KeyUp,
            0x22 => MessageType::MouseMove,
            0x23 => MessageType::MouseDown,
            0x24 => MessageType::MouseUp,
            0x25 => MessageType::MouseScroll,

            0x30 => MessageType::Clipboard,

            other => MessageType::Unknown(other),
        }
    }

    pub fn to_u8(&self) -> u8 {
        match self {
            MessageType::Text        => 0x01,
            MessageType::Connect     => 0x02,
            MessageType::Disconnect  => 0x03,
            MessageType::Error       => 0x04,

            MessageType::FrameFull   => 0x10,
            MessageType::FrameDelta  => 0x11,
            MessageType::CursorShape => 0x12,
            MessageType::CursorPos   => 0x13,
            MessageType::Resize      => 0x14,

            MessageType::KeyDown     => 0x20,
            MessageType::KeyUp       => 0x21,
            MessageType::MouseMove   => 0x22,
            MessageType::MouseDown   => 0x23,
            MessageType::MouseUp     => 0x24,
            MessageType::MouseScroll => 0x25,

            MessageType::Clipboard   => 0x30,

            MessageType::Unknown(code) => *code,
        }
    }
}