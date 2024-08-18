use std::fmt;

// Enum representing the message that can be sent over Bluetooth Low Energy
#[derive(Debug)]
pub enum BleMessage {
    Text(String),
    Raw(Vec<u8>),
}

impl BleMessage {
    /// Comsume the message and return the bytes representation of the message
    pub fn take_bytes(self) -> Vec<u8> {
        match self {
            BleMessage::Text(s) => s.as_bytes().to_vec(),
            BleMessage::Raw(v) => v,
        }
    }

    /// Get the message as a string.
    /// For Text messages, this will return the string as is.
    /// For Raw messages, this return the UTF-8 encoded string of the bytes.
    pub fn as_string(&self) -> String {
        match self {
            BleMessage::Text(s) => s.clone(),
            BleMessage::Raw(v) => String::from_utf8_lossy(v).to_string(),
        }
    }
}

impl From<&str> for BleMessage {
    /// Automatically convert a string slice to a BleMessage
    fn from(s: &str) -> Self {
        Self::Text(s.to_string())
    }
}

impl From<String> for BleMessage {
    /// Automatically convert a string to a BleMessage
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}

impl From<Vec<u8>> for BleMessage {
    /// Automatically convert a vector of bytes to a BleMessage
    fn from(value: Vec<u8>) -> Self {
        Self::Raw(value)
    }
}

impl fmt::Display for BleMessage {
    /// Display the message as a string
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            BleMessage::Text(s) => write!(f, "Text BLE Message: {}", s),
            BleMessage::Raw(v) => write!(f, "Raw BLE Message: {:?}", v),
        }
    }
}
