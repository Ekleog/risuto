use std::ops::{Deref, DerefMut};

use anyhow::Context;
use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{self, request},
};
use risuto_api::{AuthToken, UserId, Uuid};

use crate::{db, Error, UserFeeds};

#[derive(Clone, axum::extract::FromRef)]
pub struct AppState {
    pub db: PgPool,
    pub feeds: UserFeeds,
    pub admin_token: Option<AuthToken>,
}
#[derive(Clone)]
pub struct PgPool(sqlx::PgPool);

impl PgPool {
    pub fn new(pool: sqlx::PgPool) -> PgPool {
        PgPool(pool)
    }

    pub async fn acquire(&self) -> Result<PgConn, Error> {
        Ok(PgConn(
            self.0.acquire().await.context("acquiring db connection")?,
        ))
    }

    pub fn num_idle(&self) -> usize {
        self.0.num_idle()
    }
}

pub struct PgConn(sqlx::pool::PoolConnection<sqlx::Postgres>);

#[async_trait]
impl FromRequestParts<AppState> for PgConn {
    type Rejection = Error;

    async fn from_request_parts(
        _req: &mut request::Parts,
        state: &AppState,
    ) -> Result<PgConn, Error> {
        state.db.acquire().await
    }
}

impl Deref for PgConn {
    type Target = sqlx::PgConnection;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for PgConn {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub struct PreAuth(pub AuthToken);

#[async_trait]
impl<S: Sync> FromRequestParts<S> for PreAuth {
    type Rejection = Error;

    async fn from_request_parts(req: &mut request::Parts, _state: &S) -> Result<PreAuth, Error> {
        match req.headers.get(http::header::AUTHORIZATION) {
            None => Err(Error::permission_denied()),
            Some(auth) => {
                let auth = auth.to_str().map_err(|_| Error::permission_denied())?;
                let mut auth = auth.split(' ');
                if !auth
                    .next()
                    .ok_or(Error::permission_denied())?
                    .eq_ignore_ascii_case("bearer")
                {
                    return Err(Error::permission_denied());
                }
                let token = auth.next().ok_or(Error::permission_denied())?;
                if !auth.next().is_none() {
                    return Err(Error::permission_denied());
                }
                let token = Uuid::try_from(token).map_err(|_| Error::permission_denied())?;
                Ok(PreAuth(AuthToken(token)))
            }
        }
    }
}

pub struct Auth(pub UserId);

#[async_trait]
impl FromRequestParts<AppState> for Auth {
    type Rejection = Error;

    async fn from_request_parts(req: &mut request::Parts, state: &AppState) -> Result<Auth, Error> {
        let token = PreAuth::from_request_parts(req, state).await?.0;
        let mut conn = PgConn::from_request_parts(req, state).await?;
        Ok(Auth(db::recover_session(&mut *conn, token).await?))
    }
}

pub struct AdminAuth;

#[async_trait]
impl FromRequestParts<AppState> for AdminAuth {
    type Rejection = Error;

    async fn from_request_parts(
        req: &mut request::Parts,
        state: &AppState,
    ) -> Result<AdminAuth, Error> {
        let token = PreAuth::from_request_parts(req, state).await?.0;
        if Some(token) == state.admin_token {
            Ok(AdminAuth)
        } else {
            Err(Error::permission_denied())
        }
    }
}
