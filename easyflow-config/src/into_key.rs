use crate::key::Key;
use std::str::FromStr;

/// Provide the conversion to [Key] on the implemented type.
pub trait IntoKey {
    fn into(self) -> Key;
}

impl IntoKey for String {
    fn into(self) -> Key {
        Key::from_str(&self).unwrap()
    }
}

impl IntoKey for &String {
    fn into(self) -> Key {
        Key::from_str(self).unwrap()
    }
}

impl IntoKey for &str {
    fn into(self) -> Key {
        Key::from_str(self).unwrap()
    }
}

impl IntoKey for Key {
    fn into(self) -> Key {
        self
    }
}

impl IntoKey for &Key {
    fn into(self) -> Key {
        self.clone()
    }
}
