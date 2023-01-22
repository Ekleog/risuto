use risuto_api::{Error as ApiError, Uuid};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),

    #[error(transparent)]
    Api(#[from] ApiError),
}

impl Error {
    pub fn permission_denied() -> Error {
        Error::Api(ApiError::PermissionDenied)
    }

    pub fn uuid_already_used(uuid: Uuid) -> Error {
        Error::Api(ApiError::UuidAlreadyUsed(uuid))
    }

    pub fn name_already_used(name: String) -> Error {
        Error::Api(ApiError::NameAlreadyUsed(name))
    }

    pub fn invalid_pow() -> Error {
        Error::Api(ApiError::InvalidPow)
    }
}

impl axum::response::IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let err = match self {
            Error::Anyhow(err) => {
                tracing::error!(?err, "internal server error");
                #[cfg(not(test))]
                let err =
                    ApiError::Unknown(String::from("Internal server error, see logs for details"));
                #[cfg(test)]
                let err = ApiError::Unknown(format!("Internal server error: {err:?}"));
                err
            }
            Error::Api(err) => {
                tracing::info!("returning error to client: {err}");
                err
            }
        };
        (err.status_code(), err.contents()).into_response()
    }
}
