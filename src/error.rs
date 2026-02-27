use std::fmt;

#[derive(Debug)]
pub enum AppError {
    Libvirt(String),
    Xml(String),
    Io(std::io::Error),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Libvirt(msg) => write!(f, "Libvirt error: {msg}"),
            AppError::Xml(msg) => write!(f, "XML error: {msg}"),
            AppError::Io(err) => write!(f, "IO error: {err}"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<virt::error::Error> for AppError {
    fn from(err: virt::error::Error) -> Self {
        AppError::Libvirt(err.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Io(err)
    }
}

impl From<quick_xml::Error> for AppError {
    fn from(err: quick_xml::Error) -> Self {
        AppError::Xml(err.to_string())
    }
}
