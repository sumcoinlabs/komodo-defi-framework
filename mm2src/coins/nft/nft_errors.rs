use crate::eth::GetEthAddressError;
use crate::nft_storage::{CreateNftStorageError, NftStorageError};
use crate::GetMyAddressError;
use common::HttpStatusCode;
use derive_more::Display;
use enum_from::EnumFromStringify;
use http::StatusCode;
use mm2_net::transport::SlurpError;
use serde::{Deserialize, Serialize};
use web3::Error;

#[derive(Clone, Debug, Deserialize, Display, EnumFromStringify, PartialEq, Serialize, SerializeErrorType)]
#[serde(tag = "error_type", content = "error_data")]
pub enum GetNftInfoError {
    /// `http::Error` can appear on an HTTP request [`http::Builder::build`] building.
    #[from_stringify("http::Error")]
    #[display(fmt = "Invalid request: {}", _0)]
    InvalidRequest(String),
    #[display(fmt = "Transport: {}", _0)]
    Transport(String),
    #[from_stringify("serde_json::Error")]
    #[display(fmt = "Invalid response: {}", _0)]
    InvalidResponse(String),
    #[display(fmt = "Internal: {}", _0)]
    Internal(String),
    GetEthAddressError(GetEthAddressError),
    #[display(fmt = "X-API-Key is missing")]
    ApiKeyError,
    #[display(
        fmt = "Token: token_address {}, token_id {} was not find in wallet",
        token_address,
        token_id
    )]
    TokenNotFoundInWallet {
        token_address: String,
        token_id: String,
    },
    #[display(fmt = "DB error {}", _0)]
    DbError(String),
    GetMyAddressError(GetMyAddressError),
}

impl From<GetMyAddressError> for GetNftInfoError {
    fn from(e: GetMyAddressError) -> Self { GetNftInfoError::GetMyAddressError(e) }
}

impl From<SlurpError> for GetNftInfoError {
    fn from(e: SlurpError) -> Self {
        let error_str = e.to_string();
        match e {
            SlurpError::ErrorDeserializing { .. } => GetNftInfoError::InvalidResponse(error_str),
            SlurpError::Transport { .. } | SlurpError::Timeout { .. } => GetNftInfoError::Transport(error_str),
            SlurpError::Internal(_) | SlurpError::InvalidRequest(_) => GetNftInfoError::Internal(error_str),
        }
    }
}

impl From<web3::Error> for GetNftInfoError {
    fn from(e: Error) -> Self {
        let error_str = e.to_string();
        match e {
            web3::Error::InvalidResponse(_) | web3::Error::Decoder(_) | web3::Error::Rpc(_) => {
                GetNftInfoError::InvalidResponse(error_str)
            },
            web3::Error::Transport(_) | web3::Error::Io(_) => GetNftInfoError::Transport(error_str),
            _ => GetNftInfoError::Internal(error_str),
        }
    }
}

impl From<GetEthAddressError> for GetNftInfoError {
    fn from(e: GetEthAddressError) -> Self { GetNftInfoError::GetEthAddressError(e) }
}

impl From<CreateNftStorageError> for GetNftInfoError {
    fn from(e: CreateNftStorageError) -> Self {
        match e {
            CreateNftStorageError::Internal(err) => GetNftInfoError::Internal(err),
        }
    }
}

impl<T: NftStorageError> From<T> for GetNftInfoError {
    fn from(err: T) -> Self {
        let msg = format!("{:?}", err);
        GetNftInfoError::DbError(msg)
    }
}

impl From<GetNftInfoError> for UpdateNftError {
    // expand UpdateNftError::GetNftInfoError
    fn from(e: GetNftInfoError) -> Self { UpdateNftError::GetNftInfoError(e) }
}

impl HttpStatusCode for GetNftInfoError {
    fn status_code(&self) -> StatusCode {
        match self {
            GetNftInfoError::InvalidRequest(_) => StatusCode::BAD_REQUEST,
            GetNftInfoError::InvalidResponse(_) => StatusCode::FAILED_DEPENDENCY,
            GetNftInfoError::ApiKeyError => StatusCode::FORBIDDEN,
            GetNftInfoError::Transport(_)
            | GetNftInfoError::Internal(_)
            | GetNftInfoError::GetEthAddressError(_)
            | GetNftInfoError::TokenNotFoundInWallet { .. }
            | GetNftInfoError::DbError(_)
            | GetNftInfoError::GetMyAddressError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Display, EnumFromStringify, PartialEq, Serialize, SerializeErrorType)]
#[serde(tag = "error_type", content = "error_data")]
pub enum UpdateNftError {
    #[display(fmt = "DB error {}", _0)]
    DbError(String),
    #[display(fmt = "Internal: {}", _0)]
    Internal(String),
    GetNftInfoError(GetNftInfoError),
}

impl From<CreateNftStorageError> for UpdateNftError {
    fn from(e: CreateNftStorageError) -> Self {
        match e {
            CreateNftStorageError::Internal(err) => UpdateNftError::Internal(err),
        }
    }
}

impl<T: NftStorageError> From<T> for UpdateNftError {
    fn from(err: T) -> Self {
        let msg = format!("{:?}", err);
        UpdateNftError::DbError(msg)
    }
}

impl HttpStatusCode for UpdateNftError {
    fn status_code(&self) -> StatusCode {
        match self {
            // expand UpdateNftError::GetNftInfoError
            UpdateNftError::DbError(_) | UpdateNftError::Internal(_) | UpdateNftError::GetNftInfoError(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            },
        }
    }
}
