use std::sync::Arc;

use axum::extract::{FromRequestParts, Path, State};
use axum::http::{header, request::Parts, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;

use crate::db::is_unique_violation;
use crate::error::{AppError, AppResult};
use crate::slug;
use crate::state::AppState;
use crate::url;

const MAX_SLUG_ATTEMPTS: usize = 8;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/api/links", post(create_link))
        .route("/api/links/{slug}", get(get_link))
        .route("/{slug}", get(redirect))
        .layer(axum::extract::DefaultBodyLimit::max(4096))
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .with_state(state)
}

// ---------------------------------------------------------------- auth

/// Extractor that validates a bearer token or `X-API-Key` header.
pub struct ApiKey;

impl FromRequestParts<Arc<AppState>> for ApiKey {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let header_token = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| {
                let v = v.trim();
                v.strip_prefix("Bearer ")
                    .or_else(|| v.strip_prefix("bearer "))
            })
            .map(str::trim);

        let token = header_token
            .or_else(|| {
                parts
                    .headers
                    .get("x-api-key")
                    .and_then(|v| v.to_str().ok())
                    .map(str::trim)
            })
            .filter(|t| !t.is_empty())
            .ok_or(AppError::Unauthorized)?;

        let ok = state
            .cfg
            .api_keys
            .iter()
            .any(|candidate| ct_eq(candidate.as_bytes(), token.as_bytes()));

        if ok {
            Ok(ApiKey)
        } else {
            Err(AppError::Unauthorized)
        }
    }
}

fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.ct_eq(b).into()
}

// ---------------------------------------------------------------- models

#[derive(Debug, Deserialize)]
struct CreateLinkRequest {
    /// Target URL. `https://` is assumed when the scheme is omitted.
    url: String,
    /// Optional caller-chosen slug.
    #[serde(default)]
    slug: Option<String>,
}

#[derive(Debug, Serialize)]
struct LinkResponse {
    slug: String,
    short_url: String,
    url: String,
    created_at: String,
}

// ---------------------------------------------------------------- handlers

async fn create_link(
    State(state): State<Arc<AppState>>,
    _key: ApiKey,
    Json(req): Json<CreateLinkRequest>,
) -> AppResult<impl IntoResponse> {
    let target = url::normalize(&req.url)?;

    let requested = req.slug.as_deref().map(str::trim).filter(|s| !s.is_empty());

    if let Some(custom) = requested {
        if !slug::is_valid_custom(custom) {
            return Err(AppError::BadRequest(
                "slug must be 1-64 chars of [A-Za-z0-9_-] and not start with '-' or '_'".into(),
            ));
        }
        if slug::is_reserved(custom) {
            return Err(AppError::BadRequest(format!(
                "slug '{custom}' is reserved by the service"
            )));
        }
    }

    let (slug, created_at) = insert_link(&state, &target, requested).await?;

    let body = LinkResponse {
        short_url: state.cfg.short_url(&slug),
        slug,
        url: target,
        created_at,
    };
    Ok((StatusCode::CREATED, Json(body)))
}

async fn insert_link(
    state: &AppState,
    target: &str,
    requested: Option<&str>,
) -> AppResult<(String, String)> {
    for attempt in 0..MAX_SLUG_ATTEMPTS {
        let candidate = match requested {
            Some(s) => s.to_string(),
            None => slug::random(state.cfg.slug_len),
        };

        let result = sqlx::query_scalar::<_, String>(
            "INSERT INTO links (slug, url) VALUES (?1, ?2) RETURNING created_at",
        )
        .bind(&candidate)
        .bind(target)
        .fetch_one(&state.db)
        .await;

        match result {
            Ok(created_at) => return Ok((candidate, created_at)),
            Err(err) if is_unique_violation(&err) => {
                if let Some(custom) = requested {
                    return Err(AppError::Conflict(format!(
                        "slug '{custom}' is already in use"
                    )));
                }
                if attempt + 1 == MAX_SLUG_ATTEMPTS {
                    return Err(AppError::Internal(anyhow::anyhow!(
                        "could not allocate a free slug after {MAX_SLUG_ATTEMPTS} attempts"
                    )));
                }
            }
            Err(err) => return Err(AppError::Internal(err.into())),
        }
    }
    Err(AppError::Internal(anyhow::anyhow!("unreachable")))
}

async fn redirect(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> AppResult<Response> {
    if slug::is_reserved(&slug) {
        return Err(AppError::NotFound);
    }

    let target: Option<String> = sqlx::query_scalar("SELECT url FROM links WHERE slug = ?1")
        .bind(&slug)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let target = target.ok_or(AppError::NotFound)?;

    Response::builder()
        .status(StatusCode::FOUND)
        .header(header::LOCATION, target)
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::REFERRER_POLICY, "no-referrer")
        .body(axum::body::Body::empty())
        .map_err(|e| AppError::Internal(e.into()))
}

async fn get_link(
    State(state): State<Arc<AppState>>,
    _key: ApiKey,
    Path(slug): Path<String>,
) -> AppResult<Json<LinkResponse>> {
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT url, created_at FROM links WHERE slug = ?1")
            .bind(&slug)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;

    let (url, created_at) = row.ok_or(AppError::NotFound)?;
    Ok(Json(LinkResponse {
        short_url: state.cfg.short_url(&slug),
        slug,
        url,
        created_at,
    }))
}

async fn healthz(State(state): State<Arc<AppState>>) -> AppResult<Response> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM links")
        .fetch_one(&state.db)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "ok",
            "links": count,
            "base_url": state.cfg.public_base_url,
        })),
    )
        .into_response())
}
