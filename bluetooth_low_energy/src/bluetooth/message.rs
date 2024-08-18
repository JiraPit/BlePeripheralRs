use std::error::Error;
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

    /// Convert from raw bytes message to a text message.
    /// Return an error if the message is not raw bytes.
    pub fn convert_to_text(self) -> Result<Self, Box<dyn Error>> {
        match self {
            BleMessage::Raw(v) => {
                let s = String::from_utf8_lossy(&v).to_string();
                Ok(BleMessage::Text(s))
            }
            _ => Err("Message must be raw bytes in order to convert to text".into()),
        }
    }

    /// Extend the raw bytes with another byte vector.
    /// Return an error if the message is not raw bytes
    pub fn extend_raw_bytes(&mut self, bytes: Vec<u8>) -> Result<(), Box<dyn Error>> {
        match self {
            BleMessage::Raw(v) => {
                v.extend(bytes);
                Ok(())
            }
            _ => Err("Message must be raw bytes in order to extend them".into()),
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
