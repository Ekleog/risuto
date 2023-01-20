use std::str::FromStr;

use anyhow::{anyhow, Context};
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("Unknown error: {0}")]
    Unknown(String),

    #[error("Permission denied")]
    PermissionDenied,

    #[error("Uuid already used {0}")]
    UuidAlreadyUsed(Uuid),

    #[error("Invalid Proof of Work")]
    InvalidPow,

    #[error("Name already used {0}")]
    NameAlreadyUsed(String),

    #[error("Null byte in string is not allowed {0:?}")]
    NullByteInString(String),

    #[error("Invalid character in name {0:?}")]
    InvalidName(String),
}

impl Error {
    pub fn status_code(&self) -> http::StatusCode {
        use http::StatusCode;
        match self {
            Error::Unknown(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::PermissionDenied => StatusCode::FORBIDDEN,
            Error::UuidAlreadyUsed(_) => StatusCode::CONFLICT,
            Error::InvalidPow => StatusCode::BAD_REQUEST,
            Error::NameAlreadyUsed(_) => StatusCode::CONFLICT,
            Error::NullByteInString(_) => StatusCode::BAD_REQUEST,
            Error::InvalidName(_) => StatusCode::BAD_REQUEST,
        }
    }

    pub fn contents(&self) -> Vec<u8> {
        serde_json::to_vec(&match self {
            Error::Unknown(msg) => json!({
                "message": msg,
                "type": "unknown",
            }),
            Error::PermissionDenied => json!({
                "message": "permission denied",
                "type": "permission-denied",
            }),
            Error::UuidAlreadyUsed(u) => json!({
                "message": "uuid conflict",
                "type": "conflict-uuid",
                "uuid": u,
            }),
            Error::InvalidPow => json!({
                "message": "invalid proof-of-work",
                "type": "invalid-pow",
            }),
            Error::NameAlreadyUsed(n) => json!({
                "message": "name already used",
                "type": "conflict-name",
                "name": n,
            }),
            Error::NullByteInString(s) => json!({
                "message": "there was a null byte in argument string",
                "type": "null-byte",
                "string": s,
            }),
            Error::InvalidName(n) => json!({
                "message": "there was an invalid character in a user name",
                "type": "invalid-name",
                "name": n,
            }),
        })
        .expect("serializing conflict")
    }

    pub fn parse(body: &[u8]) -> anyhow::Result<Error> {
        let data: serde_json::Value =
            serde_json::from_slice(body).context("parsing error contents")?;
        Ok(
            match data
                .get("type")
                .and_then(|t| t.as_str())
                .ok_or_else(|| anyhow!("error type is not a string"))?
            {
                "unknown" => Error::Unknown(String::from(
                    data.get("message")
                        .and_then(|msg| msg.as_str())
                        .unwrap_or(""),
                )),
                "permission-denied" => Error::PermissionDenied,
                "conflict-uuid" => Error::UuidAlreadyUsed(
                    data.get("uuid")
                        .and_then(|uuid| uuid.as_str())
                        .and_then(|uuid| Uuid::from_str(uuid).ok())
                        .ok_or_else(|| anyhow!("error is a uuid conflict without a proper uuid"))?,
                ),
                "invalid-pow" => Error::InvalidPow,
                "conflict-name" => Error::NameAlreadyUsed(String::from(
                    data.get("name")
                        .and_then(|n| n.as_str())
                        .ok_or_else(|| anyhow!("error is a name conflict without a name"))?,
                )),
                "null-byte" => Error::NullByteInString(String::from(
                    data.get("string").and_then(|s| s.as_str()).ok_or_else(|| {
                        anyhow!("error is a null-byte-in-string without a string")
                    })?,
                )),
                "invalid-name" => Error::InvalidName(String::from(
                    data.get("name").and_then(|s| s.as_str()).ok_or_else(|| {
                        anyhow!("error is about an invalid name but no name was provided")
                    })?,
                )),
                _ => return Err(anyhow!("error contents has unknown type")),
            },
        )
    }
}

// TODO: fuzz-assert that any Error can round-trip to itself through JSON
