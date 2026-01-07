use std::{collections::HashMap, process::Command};

use chrono::Utc;
use reqwest::{Client, Url};
use serde::Deserialize;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

use crate::{
    auth::CredentialData,
    errors::{WatsonError, WatsonErrorKind},
    watson_err,
};

const CLIENT_ID: &str = "571128954566-ma98chaempk6lsmn469r6ls2589psv01.apps.googleusercontent.com";
const CLIENT_SECRET: &str = "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";

#[derive(Debug, Deserialize)]
pub struct GoogleRefreshTokenResponse {
    pub access_token: String,
}
#[derive(Debug, Deserialize)]
pub struct GoogleTokenResponse {
    pub access_token: String,
    pub expires_in: u64,
    pub refresh_token: String,
    pub scope: String,
    pub token_type: String,
}
impl From<GoogleTokenResponse> for CredentialData {
    fn from(value: GoogleTokenResponse) -> Self {
        Self::OAuth {
            service: crate::auth::CredentialService::Google,
            access_token: crate::auth::CredentialSecret::Decrypted(value.access_token),
            refresh_token: crate::auth::CredentialSecret::Decrypted(value.refresh_token),
            expires_at: Utc::now().timestamp() + 3600,
        }
    }
}
impl GoogleTokenResponse {
    pub fn to_credential_data(self) -> CredentialData {
        CredentialData::OAuth {
            service: crate::auth::CredentialService::Google,
            access_token: crate::auth::CredentialSecret::Decrypted(self.access_token.clone()),
            refresh_token: crate::auth::CredentialSecret::Decrypted(self.refresh_token.clone()),
            expires_at: chrono::Utc::now().timestamp() + 3600,
        }
    }
}

pub fn client_auth() -> Result<(), WatsonError> {
    let mut url = Url::parse("https://accounts.google.com/o/oauth2/v2/auth").unwrap();

    url.query_pairs_mut()
        .append_pair("client_id", CLIENT_ID)
        .append_pair("redirect_uri", "http://127.0.0.1:8000")
        .append_pair("response_type", "code")
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent")
        .append_pair(
            "scope",
            "openid email https://www.googleapis.com/auth/calendar",
        );

    Command::new("xdg-open")
        .arg(url.to_string())
        .spawn()
        .map_err(|e| watson_err!(WatsonErrorKind::CommandExecute, e.to_string()))?;

    Ok(())
}

pub async fn exchange_code_for_tokens(code: &str) -> Result<GoogleTokenResponse, WatsonError> {
    let client = Client::new();
    let redirect_uri = "http://127.0.0.1:8000";

    let mut params = HashMap::new();
    params.insert("client_id", CLIENT_ID);
    params.insert("client_secret", CLIENT_SECRET);
    params.insert("code", code);
    params.insert("grant_type", "authorization_code");
    params.insert("redirect_uri", redirect_uri);

    let resp = client
        .post("https://oauth2.googleapis.com/token")
        .form(&params)
        .send()
        .await
        .map_err(|e| watson_err!(WatsonErrorKind::HttpGetRequest, e.to_string()))?;

    if !resp.status().is_success() {
        return Err(watson_err!(
            WatsonErrorKind::GoogleAuth,
            "Failed to retrieve OAuth2 credentials."
        ));
    }

    let text = resp
        .text()
        .await
        .map_err(|e| watson_err!(WatsonErrorKind::HttpPostRequest, e.to_string()))?;

    serde_json::from_str(&text)
        .map_err(|e| watson_err!(WatsonErrorKind::Deserialization, e.to_string()))
}

pub async fn wait_for_auth_code() -> Result<String, WatsonError> {
    let listener = TcpListener::bind("127.0.0.1:8000")
        .await
        .map_err(|e| watson_err!(WatsonErrorKind::StreamBind, e.to_string()))?;

    let (mut stream, _) = listener
        .accept()
        .await
        .map_err(|e| watson_err!(WatsonErrorKind::StreamConnect, e.to_string()))?;

    let mut buffer = [0; 2048];
    stream
        .read(&mut buffer)
        .await
        .map_err(|e| watson_err!(WatsonErrorKind::StreamRead, e.to_string()))?;

    let request = String::from_utf8_lossy(&buffer);

    let first_line = request.lines().next().unwrap_or("");
    let path = first_line.split_whitespace().nth(1).unwrap_or("/");

    let url = Url::parse(&format!("http://localhost{path}"))
        .map_err(|e| watson_err!(WatsonErrorKind::UrlFormat, e.to_string()))?;

    let code = url
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.to_string())
        .ok_or(watson_err!(
            WatsonErrorKind::UndefinedAttribute,
            "No ?code parameter returned."
        ))?;

    let response = "\
    HTTP/1.1 200 OK\r\n\
    Content-Type: text/html; charset=utf-8\r\n\
    Connection: close\r\n\
    \r\n\
    <html>\
      <head><title>Login Complete</title></head>\
      <body>\
        <h2>Login complete. You may close this window.</h2>\
      </body>\
    </html>";

    stream
        .write_all(response.as_bytes())
        .await
        .map_err(|e| watson_err!(WatsonErrorKind::StreamWrite, e.to_string()))?;
    stream
        .flush()
        .await
        .map_err(|e| watson_err!(WatsonErrorKind::StreamWrite, e.to_string()))?;

    Ok(code)
}

pub struct GoogleAuth;
impl GoogleAuth {
    pub async fn refresh_credential(refresh_token: &str) -> Result<String, WatsonError> {
        let client = Client::new();
        let mut params = HashMap::new();

        params.insert("client_id", CLIENT_ID);
        params.insert("client_secret", CLIENT_SECRET);
        params.insert("refresh_token", refresh_token);
        params.insert("grant_type", "refresh_token");

        let resp = client
            .post("https://oauth2.googleapis.com/token")
            .form(&params)
            .send()
            .await
            .map_err(|e| watson_err!(WatsonErrorKind::HttpGetRequest, e.to_string()))?;

        if !resp.status().is_success() {
            return Err(watson_err!(
                WatsonErrorKind::GoogleAuth,
                "Failed to retrieve OAuth2 credentials."
            ));
        }

        let text = resp
            .text()
            .await
            .map_err(|e| watson_err!(WatsonErrorKind::HttpPostRequest, e.to_string()))?;

        let response: GoogleRefreshTokenResponse = serde_json::from_str(&text)
            .map_err(|e| watson_err!(WatsonErrorKind::Deserialization, e.to_string()))?;

        Ok(response.access_token)
    }
}
