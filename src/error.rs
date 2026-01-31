//! Error types for DFC-GUI
//!
//! Centralized error handling using snafu for ergonomic error definitions.

use snafu::Snafu;

/// Main error type for the application
#[derive(Debug, Snafu)]
pub enum Error {
    /// Invalid input or configuration
    #[snafu(display("Invalid: {message}"))]
    Invalid { message: String },

    /// IO error (file operations, network, etc.)
    #[snafu(display("IO error: {source}"))]
    Io { source: std::io::Error },

    /// JSON serialization/deserialization error
    #[snafu(display("JSON error: {source}"))]
    Json { source: serde_json::Error },

    /// TOML deserialization error
    #[snafu(display("TOML parse error: {source}"))]
    TomlDe { source: toml::de::Error },

    /// TOML serialization error
    #[snafu(display("TOML serialize error: {source}"))]
    TomlSe { source: toml::ser::Error },

    /// Channel send error
    #[snafu(display("Channel send error: {message}"))]
    ChannelSend { message: String },

    /// Service connection error
    #[snafu(display("Connection error: {message}"))]
    Connection { message: String },

    /// Command execution error
    #[snafu(display("Command error: {message}"))]
    Command { message: String },

    /// Timeout error
    #[snafu(display("Timeout: {message}"))]
    Timeout { message: String },
}

impl From<std::io::Error> for Error {
    fn from(source: std::io::Error) -> Self {
        Error::Io { source }
    }
}

impl From<serde_json::Error> for Error {
    fn from(source: serde_json::Error) -> Self {
        Error::Json { source }
    }
}

impl From<toml::de::Error> for Error {
    fn from(source: toml::de::Error) -> Self {
        Error::TomlDe { source }
    }
}

impl From<toml::ser::Error> for Error {
    fn from(source: toml::ser::Error) -> Self {
        Error::TomlSe { source }
    }
}

/// Result type alias for convenience
pub type Result<T, E = Error> = std::result::Result<T, E>;
