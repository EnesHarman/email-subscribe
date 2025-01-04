use actix_web::{web, HttpResponse, ResponseError};
use anyhow::Context;
use reqwest::StatusCode;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::telemetry::error_chain_fmt;

#[derive(Deserialize)]
pub struct Parameters {
    subscription_token: String,
}

#[tracing::instrument(name = "Confirm a pending subscriber", skip(paramerters, pool))]
pub async fn confirm(
    paramerters: web::Query<Parameters>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ConfirmError> {
    let subscriber_id = get_subscriber_id_from_token(&pool, &paramerters.subscription_token)
        .await
        .context("Failed to retrieve the subscriber id associated with the provided token.")?
        .ok_or(ConfirmError::UnknownToken)?;

    confirim_subscriber(&pool, subscriber_id)
        .await
        .context("Failed to mark subscriber as confirmed.")?;
    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(name = "Mark subscriber as confirmed", skip(subscriber_id, pool))]
pub async fn confirim_subscriber(
    pool: &PgPool,
    subscriber_id: Uuid,
) -> Result<(), StoreTokenError> {
    sqlx::query!(
        r#"UPDATE subscriptions SET status = 'confirmed' WHERE id = $1"#,
        subscriber_id
    )
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        StoreTokenError(e)
    })?;
    Ok(())
}

#[tracing::instrument(name = "Get subscriber_id from token", skip(subscription_token, pool))]
pub async fn get_subscriber_id_from_token(
    pool: &PgPool,
    subscription_token: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let result = sqlx::query!(
        r#"SELECT subscriber_id FROM subscription_tokens WHERE subscription_token = $1"#,
        subscription_token
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;
    Ok(result.map(|r| r.subscriber_id))
}

#[derive(thiserror::Error)]
pub enum ConfirmError {
    #[error("There is no subscriber associated with the provided token.")]
    UnknownToken,
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for ConfirmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for ConfirmError {
    fn status_code(&self) -> StatusCode {
        match self {
            ConfirmError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ConfirmError::UnknownToken => StatusCode::UNAUTHORIZED,
        }
    }
}

struct StoreTokenError(sqlx::Error);

impl std::fmt::Debug for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl std::error::Error for StoreTokenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl std::fmt::Display for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "A database error was encountered while trying to handle a subscription token."
        )
    }
}
