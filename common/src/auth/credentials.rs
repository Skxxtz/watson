use std::collections::HashSet;

use super::tui::CredentialBuilder;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    auth::tui::CredentialService,
    errors::{WatsonError, WatsonErrorType},
};

struct ServiceCredentialIndex {
    index_entry: keyring::Entry,
    index: HashSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credential {
    pub id: String,
    pub username: String,
    pub secret: String,
    pub label: String,
}

pub struct CredentialManager {
    service_id: String,
    index: ServiceCredentialIndex,
}
impl CredentialManager {
    const SERVICE_NAMESPACE: &'static str = "dev.skxxtz.watson";
    // HELPERS
    fn get_index(service_id: &str) -> Result<ServiceCredentialIndex, WatsonError> {
        // Retrieve Index
        let index_entry =
            keyring::Entry::new(&service_id, "__index__").map_err(|e| WatsonError {
                r#type: WatsonErrorType::CredentialEntry,
                error: e.to_string(),
            })?;
        let index: HashSet<String> = match index_entry.get_secret() {
            Ok(s) => serde_json::from_slice(&s).map_err(|e| WatsonError {
                r#type: WatsonErrorType::Deserialization,
                error: e.to_string(),
            })?,
            Err(e) if matches!(e, keyring::Error::NoEntry) => HashSet::new(),
            Err(e) => {
                return Err(WatsonError {
                    r#type: WatsonErrorType::CredentialRead,
                    error: e.to_string(),
                });
            }
        };

        Ok(ServiceCredentialIndex { index_entry, index })
    }

    pub fn new(service: &str) -> Result<Self, WatsonError> {
        let service_id = format!("{}.{}", Self::SERVICE_NAMESPACE, service);
        let index = Self::get_index(&service_id)?;
        Ok(Self { service_id, index })
    }

    pub fn store(
        &mut self,
        username: String,
        secret: String,
        label: String,
    ) -> Result<Credential, WatsonError> {
        // Retrieve Index
        let &mut ServiceCredentialIndex {
            ref index_entry,
            ref mut index,
        } = &mut self.index;

        // Create unique uid
        let mut id = Uuid::new_v4().to_string();
        while index.get(&id).is_some() {
            id = Uuid::new_v4().to_string();
        }

        // Create payload
        let payload = Credential {
            id: id.clone(),
            username,
            secret,
            label,
        };
        let payload_json = serde_json::to_vec(&payload).map_err(|e| WatsonError {
            r#type: WatsonErrorType::Serialization,
            error: e.to_string(),
        })?;

        // Set entry
        let entry = keyring::Entry::new(&self.service_id, &id).map_err(|e| WatsonError {
            r#type: WatsonErrorType::CredentialEntry,
            error: e.to_string(),
        })?;
        entry.set_secret(&payload_json).map_err(|e| WatsonError {
            r#type: WatsonErrorType::CredentialEntry,
            error: e.to_string(),
        })?;

        // Update index
        index.insert(id.clone());
        let index_payload = serde_json::to_vec(&index).map_err(|e| WatsonError {
            r#type: WatsonErrorType::Serialization,
            error: e.to_string(),
        })?;
        index_entry
            .set_secret(&index_payload)
            .map_err(|e| WatsonError {
                r#type: WatsonErrorType::CredentialEntry,
                error: e.to_string(),
            })?;

        Ok(payload)
    }

    pub fn get_credentials(&self) -> Result<Vec<Credential>, WatsonError> {
        // Retrieve Index
        let ServiceCredentialIndex { index, .. } = &self.index;

        // Parse credentials
        let creds: Vec<Credential> = index
            .into_iter()
            .filter_map(|id| {
                let entry = keyring::Entry::new(&self.service_id, &id).ok()?;
                let payload = entry.get_secret().ok()?;
                serde_json::from_slice(&payload).ok()
            })
            .collect();

        Ok(creds)
    }

    pub fn get_credential_builders(
        &self,
        service: CredentialService,
    ) -> Result<Vec<CredentialBuilder>, WatsonError> {
        Ok(self
            .get_credentials()?
            .into_iter()
            .map(|c| CredentialBuilder::from_credential(c, service))
            .collect())
    }

    pub fn update_credential(&mut self, new: CredentialBuilder) -> Result<(), WatsonError> {
        let ServiceCredentialIndex { index, .. } = &self.index;

        // Return if no id
        let id = new.id.clone().ok_or(WatsonError {
            r#type: WatsonErrorType::UndefinedAttribute,
            error: "Credential builder id is not set.".into(),
        })?;

        // Return if invalid id
        if !index.contains(&id) {
            return Err(WatsonError {
                r#type: WatsonErrorType::InvalidAttribute,
                error: format!("Credential builder id is not yet stored."),
            });
        }

        // Destructure Credential Builder
        let CredentialBuilder {
            username,
            secret,
            label,
            ..
        } = new;

        // Create payload
        let payload = Credential {
            id: id.clone(),
            username,
            secret,
            label,
        };
        let payload_json = serde_json::to_vec(&payload).map_err(|e| WatsonError {
            r#type: WatsonErrorType::Serialization,
            error: e.to_string(),
        })?;

        // Update entry
        let entry = keyring::Entry::new(&self.service_id, &id).map_err(|e| WatsonError {
            r#type: WatsonErrorType::CredentialEntry,
            error: e.to_string(),
        })?;
        entry.set_secret(&payload_json).map_err(|e| WatsonError {
            r#type: WatsonErrorType::CredentialEntry,
            error: e.to_string(),
        })?;

        Ok(())
    }
    pub fn remove_credential(&mut self, id: &str) -> Result<(), WatsonError> {
        let &mut ServiceCredentialIndex {
            ref index_entry,
            ref mut index,
        } = &mut self.index;

        // Check if the secret exists
        if !index.contains(id) {
            return Err(WatsonError {
                r#type: WatsonErrorType::CredentialRead,
                error: format!("Credential ID {} not found", id),
            });
        }

        // Delete the sectret
        let entry = keyring::Entry::new(&self.service_id, &id).map_err(|e| WatsonError {
            r#type: WatsonErrorType::CredentialEntry,
            error: e.to_string(),
        })?;
        entry.delete_credential().map_err(|e| WatsonError {
            r#type: WatsonErrorType::CredentialEntry,
            error: e.to_string(),
        })?;

        // Update index
        index.remove(id);
        let index_payload = serde_json::to_vec(&index).map_err(|e| WatsonError {
            r#type: WatsonErrorType::Serialization,
            error: e.to_string(),
        })?;
        index_entry
            .set_secret(&index_payload)
            .map_err(|e| WatsonError {
                r#type: WatsonErrorType::CredentialEntry,
                error: e.to_string(),
            })?;

        Ok(())
    }
}
