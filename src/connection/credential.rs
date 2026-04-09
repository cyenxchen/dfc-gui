//! Preset Credential Management
//!
//! Preset credentials for Redis authentication with optional ACL username support.

use serde::{Deserialize, Serialize};

/// Preset credential for Redis authentication
#[derive(Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PresetCredential {
    /// Optional username for ACL (Redis 6+)
    pub username: Option<String>,
    /// Password for authentication
    pub password: String,
}

impl PresetCredential {
    /// Parse from string, format: "password" or "username:password"
    pub fn from_str(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }
        if let Some((username, password)) = s.split_once(':') {
            Some(Self {
                username: Some(username.to_string()),
                password: password.to_string(),
            })
        } else {
            Some(Self {
                username: None,
                password: s.to_string(),
            })
        }
    }

    /// Convert to string format: "password" or "username:password"
    pub fn to_string(&self) -> String {
        match &self.username {
            Some(u) => format!("{}:{}", u, self.password),
            None => self.password.clone(),
        }
    }
}

/// Encrypted preset credential for storage
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EncryptedPresetCredential {
    pub username: Option<String>,
    pub password: String, // Encrypted password
}

/// Convert credentials to display text (one per line)
pub fn credentials_to_text(credentials: &[PresetCredential]) -> String {
    credentials
        .iter()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Parse text to credentials (one per line)
pub fn text_to_credentials(text: &str) -> Vec<PresetCredential> {
    text.lines()
        .filter_map(PresetCredential::from_str)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_password_only() {
        let cred = PresetCredential::from_str("mypassword").unwrap();
        assert_eq!(cred.username, None);
        assert_eq!(cred.password, "mypassword");
    }

    #[test]
    fn test_parse_username_password() {
        let cred = PresetCredential::from_str("admin:secret123").unwrap();
        assert_eq!(cred.username, Some("admin".to_string()));
        assert_eq!(cred.password, "secret123");
    }

    #[test]
    fn test_parse_empty() {
        assert!(PresetCredential::from_str("").is_none());
        assert!(PresetCredential::from_str("   ").is_none());
    }

    #[test]
    fn test_to_string() {
        let cred1 = PresetCredential {
            username: None,
            password: "pass".to_string(),
        };
        assert_eq!(cred1.to_string(), "pass");

        let cred2 = PresetCredential {
            username: Some("user".to_string()),
            password: "pass".to_string(),
        };
        assert_eq!(cred2.to_string(), "user:pass");
    }

    #[test]
    fn test_text_conversion() {
        let text = "pass1\nadmin:pass2\n\npass3";
        let creds = text_to_credentials(text);
        assert_eq!(creds.len(), 3);

        let back = credentials_to_text(&creds);
        assert_eq!(back, "pass1\nadmin:pass2\npass3");
    }
}
