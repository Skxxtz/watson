use std::{
    env::home_dir,
    fmt::Display,
    fs::{File, OpenOptions, create_dir_all},
    io::{self, BufReader, Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
};
use zeroize::{Zeroize, ZeroizeOnDrop};

use base64::{Engine, engine::general_purpose};
use serde::{Deserialize, Serialize};

use chacha20poly1305::{
    Key, KeyInit, XChaCha20Poly1305, XNonce,
    aead::{Aead, OsRng, rand_core::RngCore},
};

use crate::{
    errors::{WatsonError, WatsonErrorKind},
    watson_err,
};

use super::tui::CredentialService;
// Load master key
// Deserialize JSON fields

// ------- Crypto --------
fn encrypt(secret: &[u8], key: &[u8], nonce: &[u8], aad: &[u8]) -> Result<Vec<u8>, WatsonError> {
    let key = Key::from_slice(key);
    let nonce = XNonce::from_slice(&nonce);
    let cipher = XChaCha20Poly1305::new(key);
    cipher
        .encrypt(nonce, chacha20poly1305::aead::Payload { msg: secret, aad })
        .map_err(|e| watson_err!(WatsonErrorKind::Decryption, &e.to_string()))
}
fn decrypt(
    ciphertext: &[u8],
    key: &[u8],
    nonce: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, WatsonError> {
    let key = Key::from_slice(key);
    let cipher = XChaCha20Poly1305::new(key);
    let nonce = XNonce::from_slice(nonce);
    cipher
        .decrypt(
            nonce,
            chacha20poly1305::aead::Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|e| watson_err!(WatsonErrorKind::Decryption, e.to_string()))
}

// ------- Serde Types (disk only) ------------
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum CredentialSecretSerde {
    Encrypted { nonce: String, ciphertext: String },
}
impl From<&CredentialSecret> for CredentialSecretSerde {
    fn from(value: &CredentialSecret) -> Self {
        match value {
            CredentialSecret::Encrypted { nonce, ciphertext } => CredentialSecretSerde::Encrypted {
                nonce: general_purpose::STANDARD.encode(nonce),
                ciphertext: general_purpose::STANDARD.encode(ciphertext),
            },
            CredentialSecret::Decrypted(_) => {
                panic!("Attempted to serialize decrypted secred")
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialSerde {
    pub id: String,
    pub service: CredentialService,
    pub username: CredentialSecretSerde,
    pub secret: CredentialSecretSerde,
    pub label: String,
}

impl From<Credential> for CredentialSerde {
    fn from(v: Credential) -> Self {
        Self {
            id: v.id,
            service: v.service,
            username: (&v.username).into(),
            secret: (&v.secret).into(),
            label: v.label,
        }
    }
}

// ------- Runtime-Only Types ---------
#[derive(Debug, Clone)]
pub enum CredentialSecret {
    Encrypted {
        nonce: [u8; 24],
        ciphertext: Vec<u8>,
    },
    Decrypted(String),
}
impl CredentialSecret {
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Decrypted(t) => t.is_empty(),
            Self::Encrypted { nonce, ciphertext } => nonce.is_empty() || ciphertext.is_empty(),
        }
    }
    pub fn is_locked(&self) -> bool {
        match self {
            Self::Decrypted(_) => false,
            Self::Encrypted { .. } => true,
        }
    }
}
impl ZeroizeOnDrop for CredentialSecret {}
impl Zeroize for CredentialSecret {
    fn zeroize(&mut self) {
        match self {
            CredentialSecret::Encrypted { nonce, ciphertext } => {
                nonce.zeroize();
                ciphertext.zeroize();
            }
            CredentialSecret::Decrypted(s) => {
                s.zeroize();
            }
        }
    }
}

impl TryFrom<CredentialSecretSerde> for CredentialSecret {
    type Error = WatsonError;

    fn try_from(s: CredentialSecretSerde) -> Result<Self, Self::Error> {
        match s {
            CredentialSecretSerde::Encrypted { nonce, ciphertext } => {
                let nonce_bytes = general_purpose::STANDARD.decode(nonce).map_err(|_| {
                    watson_err!(WatsonErrorKind::Base64Decode, "Invalid nonce encoding")
                })?;

                let ciphertext_bytes =
                    general_purpose::STANDARD.decode(ciphertext).map_err(|_| {
                        watson_err!(WatsonErrorKind::Base64Decode, "Invalid ciphertext encoding")
                    })?;

                let nonce: [u8; 24] = nonce_bytes.try_into().map_err(|_| {
                    watson_err!(WatsonErrorKind::Base64Decode, "Invalid nonce length")
                })?;

                Ok(CredentialSecret::Encrypted {
                    nonce,
                    ciphertext: ciphertext_bytes,
                })
            }
        }
    }
}

impl Display for CredentialSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Encrypted { .. } => write!(f, "<encrypted>"),
            Self::Decrypted(text) => write!(f, "{text}"),
        }
    }
}

impl Default for CredentialSecret {
    fn default() -> Self {
        Self::Decrypted(String::new())
    }
}

#[derive(Debug, Clone)]
pub struct Credential {
    pub id: String,
    pub service: CredentialService,
    pub username: CredentialSecret,
    pub secret: CredentialSecret,
    pub label: String,
}

impl Credential {
    pub fn new(
        username: String,
        secret: String,
        service: CredentialService,
        label: String,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            service,
            username: CredentialSecret::Decrypted(username),
            secret: CredentialSecret::Decrypted(secret),
            label,
        }
    }
    pub fn unlock(&mut self, key: &[u8]) -> Result<(), WatsonError> {
        let aad = format!("{}:{}", self.service, self.id);
        if let CredentialSecret::Encrypted { nonce, ciphertext } = &self.username {
            let decrypted_bytes = decrypt(ciphertext, key, nonce, aad.as_bytes())?;
            let decrypted = String::from_utf8(decrypted_bytes)
                .map_err(|e| watson_err!(WatsonErrorKind::Deserialization, e.to_string()))?;

            self.username = CredentialSecret::Decrypted(decrypted);
        }

        if let CredentialSecret::Encrypted { nonce, ciphertext } = &self.secret {
            let decrypted_bytes = decrypt(ciphertext, key, nonce, aad.as_bytes())?;
            let decrypted = String::from_utf8(decrypted_bytes)
                .map_err(|e| watson_err!(WatsonErrorKind::Deserialization, e.to_string()))?;
            self.secret = CredentialSecret::Decrypted(decrypted);
        }

        Ok(())
    }
    pub fn lock(&mut self, key: &[u8]) -> Result<(), WatsonError> {
        let aad = format!("{}:{}", self.service, self.id);
        let encrypt_field = |field: &mut CredentialSecret| -> Result<(), WatsonError> {
            if let CredentialSecret::Decrypted(secret) = field {
                let mut nonce_bytes = [0u8; 24];
                OsRng.fill_bytes(&mut nonce_bytes);

                let encrypted = encrypt(secret.as_bytes(), key, &nonce_bytes, aad.as_bytes())?;
                *field = CredentialSecret::Encrypted {
                    ciphertext: encrypted,
                    nonce: nonce_bytes,
                };
            }
            Ok(())
        };

        encrypt_field(&mut self.username)?;
        encrypt_field(&mut self.secret)?;

        Ok(())
    }
}

impl TryFrom<CredentialSerde> for Credential {
    type Error = WatsonError;

    fn try_from(c: CredentialSerde) -> Result<Self, Self::Error> {
        Ok(Self {
            id: c.id,
            service: c.service,
            username: c.username.try_into()?,
            secret: c.secret.try_into()?,
            label: c.label,
        })
    }
}

// -------- Interface --------------
pub struct CredentialManager {
    pub key: [u8; 32],
    pub credentials: Vec<Credential>,
}
impl CredentialManager {
    pub fn paths() -> Result<(PathBuf, PathBuf), WatsonError> {
        let home =
            home_dir().ok_or_else(|| watson_err!(WatsonErrorKind::EnvVar, "HOME not set."))?;

        let cred = home.join(".watson/credentials.json");
        let key = home.join(".watson/master.key");

        Ok((cred, key))
    }
    pub fn new() -> Result<Self, WatsonError> {
        let (cred_path, key_path) = Self::paths()?;

        // Create parent dir if it doesnt exist
        if let Some(p) = cred_path.parent() {
            if !p.exists() {
                create_dir_all(p)
                    .map_err(|e| watson_err!(WatsonErrorKind::DirCreate, e.to_string()))?;
            }
        }

        // Create key if it doesnt exist
        let mut key = [0u8; 32];
        match File::open(&key_path) {
            Ok(mut file) => {
                let meta = file
                    .metadata()
                    .map_err(|e| watson_err!(WatsonErrorKind::FileMetadata, e.to_string()))?;
                if meta.len() != 32 {
                    return Err(watson_err!(WatsonErrorKind::FileRead, "Invalid key length"));
                }
                file.read_exact(&mut key)
                    .map_err(|e| watson_err!(WatsonErrorKind::FileRead, e.to_string()))?;
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                // Fill key
                OsRng.fill_bytes(&mut key);

                // Save key
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(&key_path)
                    .map_err(|e| watson_err!(WatsonErrorKind::FileCreate, e.to_string()))?;
                file.write_all(&key)
                    .map_err(|e| watson_err!(WatsonErrorKind::FileWrite, e.to_string()))?;

                file.sync_all()
                    .map_err(|e| watson_err!(WatsonErrorKind::FileWrite, e.to_string()))?;
            }
            Err(e) => {
                return Err(watson_err!(WatsonErrorKind::FileRead, e.to_string()));
            }
        }

        // Create credentials
        let credentials: Vec<Credential> = match File::open(&cred_path) {
            Ok(file) => {
                let reader = BufReader::new(file);
                let credentials: Vec<CredentialSerde> = serde_json::from_reader(reader)
                    .map_err(|e| watson_err!(WatsonErrorKind::Deserialization, e.to_string()))?;

                credentials
                    .into_iter()
                    .map(TryInto::try_into)
                    .collect::<Result<Vec<_>, WatsonError>>()?
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let mut file = File::create_new(cred_path)
                    .map_err(|e| watson_err!(WatsonErrorKind::FileCreate, e.to_string()))?;
                file.write_all(b"[]")
                    .map_err(|e| watson_err!(WatsonErrorKind::FileWrite, e.to_string()))?;
                Vec::new()
            }
            Err(e) => {
                return Err(watson_err!(WatsonErrorKind::FileWrite, e.to_string()));
            }
        };

        Ok(Self { key, credentials })
    }
    pub fn lock(&mut self) -> Result<(), WatsonError> {
        for credential in &mut self.credentials {
            credential.lock(&self.key)?;
        }

        Ok(())
    }
    pub fn unlock(&mut self) -> Result<(), WatsonError> {
        self.credentials
            .iter_mut()
            .filter(|c| c.secret.is_locked() || c.username.is_locked())
            .map(|c| c.unlock(&self.key))
            .collect()
    }
    pub fn save(&mut self) -> Result<(), WatsonError> {
        let (cred_path, _) = Self::paths()?;
        // Encrypt credentials
        self.lock()?;

        let mut file = File::create(&cred_path)
            .map_err(|e| watson_err!(WatsonErrorKind::FileWrite, e.to_string()))?;

        let payload: Vec<CredentialSerde> =
            self.credentials.iter().cloned().map(Into::into).collect();
        let json = serde_json::to_vec(&payload)
            .map_err(|e| watson_err!(WatsonErrorKind::Serialization, e.to_string()))?;

        file.write_all(&json)
            .map_err(|e| watson_err!(WatsonErrorKind::FileWrite, e.to_string()))?;

        self.unlock()?;

        Ok(())
    }
    pub fn credentials_mut(&mut self) -> Result<&mut Vec<Credential>, WatsonError> {
        self.unlock()?;
        Ok(self.credentials.as_mut())
    }
    pub fn credentials(&mut self) -> Result<&Vec<Credential>, WatsonError> {
        self.unlock()?;
        Ok(self.credentials.as_ref())
    }
    pub fn insert(&mut self, cred: Credential) {
        self.credentials.push(cred);
    }
    pub fn delete(&mut self, id: &str) -> Option<Credential> {
        let index = self.credentials.iter().position(|c| c.id == id)?;

        Some(self.credentials.remove(index))
    }
    pub fn delete_index(&mut self, index: usize) -> Option<Credential> {
        if index >= self.credentials.len() {
            return None;
        }
        Some(self.credentials.remove(index))
    }
}
