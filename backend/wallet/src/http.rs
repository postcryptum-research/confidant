//! The HTTP surface: thin handlers over [`Store`].

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use confidant_core::Error as CoreError;
use serde::Deserialize;

use crate::store::{Built, LogSource, Store, StoreError, WalletSummary};

/// Largest body any wallet route accepts: a transfer request names up to eight recipients.
pub const BODY_LIMIT: usize = 64 * 1024;
/// A request is cut after this long (a sync or a proof is seconds; a hung log source is not).
pub const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);
/// Requests served concurrently; proving is CPU-bound, so the cap is small.
pub const MAX_IN_FLIGHT: usize = 16;

pub struct ApiError(pub StoreError);

impl From<StoreError> for ApiError {
    fn from(e: StoreError) -> Self {
        ApiError(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self.0 {
            StoreError::NotFound => StatusCode::NOT_FOUND,
            StoreError::Full(_) => StatusCode::SERVICE_UNAVAILABLE,
            StoreError::Core(e) => match e {
                CoreError::ZeroAmount
                | CoreError::AmountOverflow
                | CoreError::InsufficientFunds { .. }
                | CoreError::TooManyOutputs(_)
                | CoreError::InvalidEncoding => StatusCode::BAD_REQUEST,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            },
            StoreError::Log(_) => StatusCode::BAD_GATEWAY,
            StoreError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (
            status,
            Json(serde_json::json!({ "error": self.0.to_string() })),
        )
            .into_response()
    }
}

type Result<T> = std::result::Result<Json<T>, ApiError>;

#[derive(Deserialize)]
pub struct Shield {
    pub amount: u64,
}

#[derive(Deserialize)]
pub struct Transfer {
    pub recipient: String,
    pub amount: u64,
}

#[derive(Deserialize)]
pub struct Unshield {
    pub amount: u64,
    pub destination: String,
}

#[derive(Deserialize)]
pub struct Pof {
    pub amount: u64,
    /// The lender's canonical challenge string (`confidant/pof/v1|…`).
    pub challenge: String,
}

pub fn router<L: LogSource + 'static>(store: Arc<Store<L>>) -> Router {
    Router::new()
        .route("/health", get(health::<L>))
        .route("/wallets", post(create::<L>))
        .route("/wallets/{id}", get(summary::<L>))
        .route("/wallets/{id}/sync", post(sync::<L>))
        .route("/wallets/{id}/shield", post(shield::<L>))
        .route("/wallets/{id}/transfer", post(transfer::<L>))
        .route("/wallets/{id}/unshield", post(unshield::<L>))
        .route("/wallets/{id}/pof", post(pof::<L>))
        // Ingress hardening (ticket "Self-security", part 3): small bodies, a request timeout, a
        // concurrency cap, and a panic answered with 500 instead of a dropped connection.
        .layer(axum::extract::DefaultBodyLimit::max(BODY_LIMIT))
        .layer(tower_http::catch_panic::CatchPanicLayer::new())
        .layer(tower_http::timeout::TimeoutLayer::with_status_code(
            axum::http::StatusCode::REQUEST_TIMEOUT,
            REQUEST_TIMEOUT,
        ))
        .layer(tower::limit::ConcurrencyLimitLayer::new(MAX_IN_FLIGHT))
        .with_state(store)
}

async fn health<L: LogSource>(State(s): State<Arc<Store<L>>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "ok": true, "wallets": s.len() }))
}

async fn create<L: LogSource>(State(s): State<Arc<Store<L>>>) -> Result<WalletSummary> {
    Ok(Json(s.create()?))
}

async fn summary<L: LogSource>(
    State(s): State<Arc<Store<L>>>,
    Path(id): Path<String>,
) -> Result<WalletSummary> {
    // A read also catches up, so the UI never shows a stale balance.
    Ok(Json(s.sync(&id).await?))
}

async fn sync<L: LogSource>(
    State(s): State<Arc<Store<L>>>,
    Path(id): Path<String>,
) -> Result<WalletSummary> {
    Ok(Json(s.sync(&id).await?))
}

async fn shield<L: LogSource>(
    State(s): State<Arc<Store<L>>>,
    Path(id): Path<String>,
    Json(b): Json<Shield>,
) -> Result<Built> {
    Ok(Json(s.build_shield(&id, b.amount).await?))
}

async fn transfer<L: LogSource>(
    State(s): State<Arc<Store<L>>>,
    Path(id): Path<String>,
    Json(b): Json<Transfer>,
) -> Result<Built> {
    Ok(Json(s.build_transfer(&id, &b.recipient, b.amount).await?))
}

async fn unshield<L: LogSource>(
    State(s): State<Arc<Store<L>>>,
    Path(id): Path<String>,
    Json(b): Json<Unshield>,
) -> Result<Built> {
    Ok(Json(s.build_unshield(&id, b.amount, &b.destination).await?))
}

async fn pof<L: LogSource>(
    State(s): State<Arc<Store<L>>>,
    Path(id): Path<String>,
    Json(b): Json<Pof>,
) -> Result<Built> {
    Ok(Json(
        s.build_proof_of_funds(&id, b.amount, &b.challenge).await?,
    ))
}
