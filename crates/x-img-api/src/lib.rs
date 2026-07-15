// SPDX-License-Identifier: MPL-2.0
//! Axum composition boundary for a future host-managed API service.
//!
//! A host must validate its session before injecting an authenticated context.
//! This crate never parses cookies, passwords, or session tokens.

use std::{
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    Extension, Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use x_img_core::{
    host_context::{AuthenticatedHostContext, XIMG_ACCESS},
    viewed_media::{CapturePlan, CapturePlanError, CapturePlanRequest, CapturePlanService},
};

type CapturePlans = Arc<Mutex<CapturePlanService>>;

/// Returns the product router. Health is public; every product API route needs
/// a host-injected, authorized context.
pub fn router() -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/context", get(context))
        .route("/api/extension/v1/capture-plans", post(capture_plan))
        .with_state(None::<CapturePlans>)
}

/// Returns a host composition with a configured, server-side capture policy.
///
/// The browser still needs an injected, authorized Monas/Synoptikon host
/// context.  An unconfigured router deliberately refuses capture requests.
pub fn router_with_capture_plans(capture_plans: CapturePlanService) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/context", get(context))
        .route("/api/extension/v1/capture-plans", post(capture_plan))
        .with_state(Some(Arc::new(Mutex::new(capture_plans))))
}

async fn health() -> &'static str {
    "x-img API scaffold"
}

async fn context(
    context: Option<Extension<AuthenticatedHostContext>>,
) -> Result<StatusCode, StatusCode> {
    let context = context.ok_or(StatusCode::UNAUTHORIZED)?.0;
    if !context.permits(XIMG_ACCESS) {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn capture_plan(
    State(capture_plans): State<Option<CapturePlans>>,
    context: Option<Extension<AuthenticatedHostContext>>,
    Json(request): Json<CapturePlanRequest>,
) -> Result<Json<CapturePlan>, StatusCode> {
    let context = context.ok_or(StatusCode::UNAUTHORIZED)?.0;
    if !context.permits(XIMG_ACCESS) {
        return Err(StatusCode::FORBIDDEN);
    }
    let capture_plans = capture_plans.ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .as_secs();
    let mut capture_plans = capture_plans
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    capture_plans
        .plan(context.actor_id(), now, request)
        .map(Json)
        .map_err(capture_plan_status)
}

fn capture_plan_status(error: CapturePlanError) -> StatusCode {
    match error {
        CapturePlanError::PairingActorMismatch
        | CapturePlanError::UnknownPairing
        | CapturePlanError::PairingExpired
        | CapturePlanError::PairingRevoked => StatusCode::FORBIDDEN,
        CapturePlanError::Scheduler => StatusCode::SERVICE_UNAVAILABLE,
        CapturePlanError::InvalidRequest
        | CapturePlanError::SiteNotEnabled
        | CapturePlanError::AdapterMismatch
        | CapturePlanError::CaptureNotEligible
        | CapturePlanError::CandidateBudgetExceeded => StatusCode::UNPROCESSABLE_ENTITY,
    }
}

#[cfg(test)]
mod tests {
    use axum::{
        Extension,
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    use x_img_core::{
        host_context::{HostContextAdapter, MonasHostContextAdapter},
        viewed_media::{
            AdapterKind, CAPTURE_REQUEST_SCHEMA_VERSION, CaptureKind, CapturePairing,
            CapturePlanRequest, CapturePlanService, SiteCapturePolicy,
        },
    };

    use super::{router, router_with_capture_plans};

    fn capture_plans() -> CapturePlanService {
        CapturePlanService::new(
            [CapturePairing {
                pairing_id: "pair-0".into(),
                actor_id: "synthetic-monas-user".into(),
                expires_at: u64::MAX,
                revoked: false,
            }],
            [SiteCapturePolicy {
                site_id: "synthetic-site".into(),
                origin: "https://example.invalid".into(),
                capture_enabled: true,
                adapter_kind: AdapterKind::ExperimentalGeneric,
                adapter_version: "1.0.0".into(),
                allow_observed_thumbnails: true,
                allow_explicit_originals: false,
                max_candidates_per_page: 2,
            }],
        )
    }

    fn request_body() -> Body {
        Body::from(
            serde_json::to_vec(&CapturePlanRequest {
                schema_version: CAPTURE_REQUEST_SCHEMA_VERSION.into(),
                pairing_id: "pair-0".into(),
                origin: "https://example.invalid".into(),
                page_url: "https://example.invalid/gallery?private=redacted".into(),
                adapter_kind: AdapterKind::ExperimentalGeneric,
                adapter_version: "1.0.0".into(),
                capture_kind: CaptureKind::ObservedThumbnail,
                media_url: "https://example.invalid/thumbnail.webp?signature=redacted".into(),
                width: 320,
                height: 200,
            })
            .expect("synthetic request serializes"),
        )
    }

    #[test]
    fn creates_a_router_without_starting_a_listener() {
        let _router = router();
    }

    #[tokio::test]
    async fn privileged_route_rejects_direct_access_and_accepts_host_context() {
        let direct = router()
            .oneshot(
                Request::builder()
                    .uri("/context")
                    .body(Body::empty())
                    .expect("request must build"),
            )
            .await
            .expect("router is infallible");
        assert_eq!(direct.status(), StatusCode::UNAUTHORIZED);

        let context = MonasHostContextAdapter
            .authenticate(include_bytes!(
                "../../../fixtures/host-context/v1/monas-valid.json"
            ))
            .expect("synthetic host context is valid");
        let admitted = router()
            .layer(Extension(context))
            .oneshot(
                Request::builder()
                    .uri("/context")
                    .body(Body::empty())
                    .expect("request must build"),
            )
            .await
            .expect("router is infallible");
        assert_eq!(admitted.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn capture_plan_requires_host_context_and_never_receives_payload_bytes() {
        let direct = router_with_capture_plans(capture_plans())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/extension/v1/capture-plans")
                    .header("content-type", "application/json")
                    .body(request_body())
                    .expect("request must build"),
            )
            .await
            .expect("router is infallible");
        assert_eq!(direct.status(), StatusCode::UNAUTHORIZED);

        let context = MonasHostContextAdapter
            .authenticate(include_bytes!(
                "../../../fixtures/host-context/v1/monas-valid.json"
            ))
            .expect("synthetic host context is valid");
        let admitted = router_with_capture_plans(capture_plans())
            .layer(Extension(context))
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/extension/v1/capture-plans")
                    .header("content-type", "application/json")
                    .body(request_body())
                    .expect("request must build"),
            )
            .await
            .expect("router is infallible");
        assert_eq!(admitted.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn default_router_fails_open_for_unconfigured_capture_policy() {
        let context = MonasHostContextAdapter
            .authenticate(include_bytes!(
                "../../../fixtures/host-context/v1/monas-valid.json"
            ))
            .expect("synthetic host context is valid");
        let response = router()
            .layer(Extension(context))
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/extension/v1/capture-plans")
                    .header("content-type", "application/json")
                    .body(request_body())
                    .expect("request must build"),
            )
            .await
            .expect("router is infallible");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
