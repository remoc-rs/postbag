use std::fmt::{Display, Formatter};

/// Error of Postbag operations.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// [`deserialize_any`](serde::de::Deserializer::deserialize_any) is unsupported
    DeserializeAnyUnsupported,
    /// End of block
    EndOfBlock,
    /// Found a varint that didn't terminate
    BadVarint,
    /// Found an invalid bool
    BadBool,
    /// Found an invalid UTF-8 char
    BadChar,
    /// Found an invalid UTF-8 string
    BadString,
    /// Found an invalid Option discriminant
    BadOption,
    /// Found an invalid enum discriminant
    BadEnum,
    /// Bad length of a sequence or map
    BadLen,
    /// Bad identifier
    BadIdentifier,
    /// Recursion limit exceeded while deserializing deeply nested data
    RecursionLimit,
    /// Overflow of target usize
    UsizeOverflow,
    /// Serde custom error
    Custom(String),
    /// I/O error.
    Io(std::io::Error),
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<Error> for std::io::Error {
    fn from(err: Error) -> Self {
        use std::io::ErrorKind;

        if let Error::Io(err) = err {
            return err;
        }

        let kind = match &err {
            Error::DeserializeAnyUnsupported => ErrorKind::Unsupported,
            Error::EndOfBlock => ErrorKind::UnexpectedEof,
            _ => ErrorKind::InvalidData,
        };

        std::io::Error::new(kind, err)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use Error::*;
        match self {
            DeserializeAnyUnsupported => write!(f, "deserialize_any is unsupported"),
            EndOfBlock => write!(f, "end of block"),
            BadVarint => write!(f, "invalid integer"),
            BadBool => write!(f, "invalid bool"),
            BadChar => write!(f, "invalid char"),
            BadString => write!(f, "invalid string"),
            BadOption => write!(f, "invalid option"),
            BadIdentifier => write!(f, "invalid identifier"),
            RecursionLimit => write!(f, "recursion limit exceeded"),
            BadEnum => write!(f, "invalid enum discriminant"),
            BadLen => write!(f, "invalid length"),
            UsizeOverflow => write!(f, "usize overflow"),
            Custom(msg) => write!(f, "serde error: {msg}"),
            Io(err) => write!(f, "IO error: {err}"),
        }
    }
}

impl serde::ser::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        Error::Custom(msg.to_string())
    }
}

impl serde::de::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        Error::Custom(msg.to_string())
    }
}

impl std::error::Error for Error {}

/// Result of Postbag operations.
pub type Result<T> = std::result::Result<T, Error>;
