use crate::ident::Ident;

/// Provide the conversion to [Ident] on the implemented type.
pub trait IntoIdent {
    fn into(self) -> Ident;
}

impl IntoIdent for String {
    fn into(self) -> Ident {
        Ident::new(&self).unwrap_or_else(|| panic!("invalid identifier '{}'", self))
    }
}

impl IntoIdent for &String {
    fn into(self) -> Ident {
        Ident::new(self).unwrap_or_else(|| panic!("invalid identifier '{}'", self))
    }
}

impl IntoIdent for &str {
    fn into(self) -> Ident {
        Ident::new(self).unwrap_or_else(|| panic!("invalid identifier '{}'", self))
    }
}

impl IntoIdent for Ident {
    fn into(self) -> Ident {
        self
    }
}

impl IntoIdent for &Ident {
    fn into(self) -> Ident {
        self.clone()
    }
}
