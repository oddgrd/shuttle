use crate::{
    error::Error,
    user::{AccountName, AccountTier, Admin, Key, User},
};
use axum::{
    extract::{Path, State},
    headers::{Cookie, HeaderMapExt},
    Json,
};
use axum_sessions::extractors::{ReadableSession, WritableSession};
use http::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};
use shuttle_common::{backends::auth::Claim, models::auth};
use tracing::instrument;

use super::{
    builder::{CacheManagerState, KeyManagerState, UserManagerState},
    RouterState,
};

#[instrument(skip(user_manager))]
pub(crate) async fn get_user(
    _: Admin,
    State(user_manager): State<UserManagerState>,
    Path(account_name): Path<AccountName>,
) -> Result<Json<auth::UserResponse>, Error> {
    let user = user_manager.get_user(account_name).await?;

    Ok(Json(user.into()))
}

#[instrument(skip(user_manager))]
pub(crate) async fn post_user(
    _: Admin,
    State(user_manager): State<UserManagerState>,
    Path((account_name, account_tier)): Path<(AccountName, AccountTier)>,
) -> Result<Json<auth::UserResponse>, Error> {
    let user = user_manager.create_user(account_name, account_tier).await?;

    Ok(Json(user.into()))
}

pub(crate) async fn login(
    mut session: WritableSession,
    State(user_manager): State<UserManagerState>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<auth::UserResponse>, Error> {
    let user = user_manager.get_user(request.account_name).await?;

    session
        .insert("account_name", user.name.clone())
        .expect("to set account name");
    session
        .insert("account_tier", user.account_tier)
        .expect("to set account tier");

    Ok(Json(user.into()))
}

pub(crate) async fn logout(
    mut session: WritableSession,
    State(cache_manager): State<CacheManagerState>,
    headers: HeaderMap,
) {
    // TODO: this is a POC, needs refactor and error handling.
    let cache_key = if let Ok(Some(cookie)) = headers.typed_try_get::<Cookie>() {
        if let Some(id) = cookie.get("shuttle.sid") {
            Some(id.to_string())
        } else {
            None
        }
    } else {
        None
    }
    .unwrap();

    // Clear the session's associated JWT from the cache.
    cache_manager.invalidate(&cache_key);

    session.destroy();
}

pub(crate) async fn convert_cookie(
    session: ReadableSession,
    State(key_manager): State<KeyManagerState>,
) -> Result<Json<shuttle_common::backends::auth::ConvertResponse>, StatusCode> {
    let account_name: String = session
        .get("account_name")
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let account_tier: AccountTier = session
        .get("account_tier")
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let claim = Claim::new(account_name, account_tier.into());

    let token = claim.into_token(key_manager.private_key())?;

    let response = shuttle_common::backends::auth::ConvertResponse { token };

    Ok(Json(response))
}

/// Convert a valid API-key bearer token to a JWT.
pub(crate) async fn convert_key(
    State(RouterState {
        key_manager,
        user_manager,
        ..
    }): State<RouterState>,
    key: Key,
) -> Result<Json<shuttle_common::backends::auth::ConvertResponse>, StatusCode> {
    let User {
        name, account_tier, ..
    } = user_manager
        .get_user_by_key(key.clone())
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let claim = Claim::new(name.to_string(), account_tier.into());

    let token = claim.into_token(key_manager.private_key())?;

    let response = shuttle_common::backends::auth::ConvertResponse { token };

    Ok(Json(response))
}

pub(crate) async fn refresh_token() {}

pub(crate) async fn get_public_key(State(key_manager): State<KeyManagerState>) -> Vec<u8> {
    key_manager.public_key().to_vec()
}

#[derive(Deserialize, Serialize)]
pub struct LoginRequest {
    account_name: AccountName,
}
