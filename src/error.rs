use std::borrow::Cow;
use std::error::Error as StdError;
use std::fmt;

pub struct Error(Box<ErrorInner>);

impl Error {
    pub fn new<M>(message: M, cause: Box<dyn StdError>) -> Self
    where
        M: Into<Cow<'static, str>>,
    {
        Error(Box::new(ErrorInner {
            message: message.into(),
            cause: Some(cause),
        }))
    }

    pub fn msg<M>(message: M) -> Self
    where
        M: Into<Cow<'static, str>>,
    {
        Error(Box::new(ErrorInner {
            message: message.into(),
            cause: None,
        }))
    }
}

#[derive(Debug)]
struct ErrorInner {
    message: Cow<'static, str>,
    cause: Option<Box<dyn StdError>>,
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <ErrorInner as fmt::Debug>::fmt(&self.0, f)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.message)
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.0.cause.as_deref().map(|e| e as &dyn StdError)
    }
}
