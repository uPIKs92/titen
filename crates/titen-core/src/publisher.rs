//! Centralized publish orchestration (#250 follow-up, thread-bundle Phase 1).
//!
//! One place maps a publish request (media type + inputs) onto the
//! `ThreadsClient` primitives. `posts.rs` (instant publish), `scheduler.rs`
//! (scheduled publish) and the upcoming thread-bundle executor all call this
//! instead of maintaining their own `match media_type` blocks.

use crate::threads_client::ThreadsClient;
use crate::{Result, TitenError, models::Account};

/// Everything needed to publish exactly one Threads post.
#[derive(Debug, Clone, Default)]
pub struct PublishRequest {
    /// `TEXT`, `IMAGE`, `VIDEO`, or `CAROUSEL`.
    pub media_type: String,
    pub caption: Option<String>,
    /// IMAGE: single URL. VIDEO: single URL. CAROUSEL: 2–20 URLs.
    pub media_urls: Vec<String>,
    pub alt_text: Option<String>,
    pub location_id: Option<String>,
    /// Publish as a reply to this existing Threads post ID.
    pub reply_to_id: Option<String>,
}

impl PublishRequest {
    /// Build from a stored `Schedule` row (media_urls is a JSON string there).
    pub fn from_schedule(
        media_type: &str,
        caption: Option<&str>,
        media_urls_json: Option<&str>,
        location_id: Option<&str>,
        reply_to_id: Option<&str>,
    ) -> Self {
        let media_urls: Vec<String> = media_urls_json
            .and_then(|u| serde_json::from_str(u).ok())
            .unwrap_or_default();
        Self {
            media_type: media_type.to_string(),
            caption: caption.map(|s| s.to_string()),
            media_urls,
            alt_text: None,
            location_id: location_id.map(|s| s.to_string()),
            reply_to_id: reply_to_id.map(|s| s.to_string()),
        }
    }

    fn image_url(&self) -> Option<&str> {
        self.media_urls.first().map(|s| s.as_str())
    }

    fn video_url(&self) -> Option<&str> {
        self.media_urls.first().map(|s| s.as_str())
    }
}

/// Validate publish-request invariants (shared by API handlers & scheduler).
///
/// Returns `Err(message)` when the request cannot be published.
pub fn validate(req: &PublishRequest) -> Result<()> {
    match req.media_type.as_str() {
        "TEXT" => {}
        "IMAGE" => {
            let url = req.image_url().filter(|u| !u.is_empty()).ok_or_else(|| {
                TitenError::InvalidRequest("image_url is required for IMAGE posts".to_string())
            })?;
            validate_media_url(url)?;
        }
        "VIDEO" => {
            let url = req.video_url().filter(|u| !u.is_empty()).ok_or_else(|| {
                TitenError::InvalidRequest("video_url is required for VIDEO posts".to_string())
            })?;
            validate_media_url(url)?;
        }
        "CAROUSEL" => {
            if req.media_urls.len() < 2 || req.media_urls.len() > 20 {
                return Err(TitenError::InvalidRequest(format!(
                    "CAROUSEL requires 2-20 image_urls, got {}",
                    req.media_urls.len()
                )));
            }
            for url in &req.media_urls {
                validate_media_url(url)?;
            }
        }
        other => {
            return Err(TitenError::InvalidRequest(format!(
                "Unsupported media type: {other}"
            )));
        }
    }
    Ok(())
}

/// SSRF / scheme validation shared by the API, MCP and scheduler (#P3.8 rules):
/// HTTPS only, no internal/private hosts.
pub fn validate_media_url(url: &str) -> Result<()> {
    validate_media_url_strict(url).map_err(TitenError::InvalidRequest)
}

/// Validate that a URL is HTTPS and well-formed.
/// Meta Threads API requires HTTPS for all media URLs.
fn validate_media_url_strict(url: &str) -> std::result::Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|e| format!("Invalid URL: {e}"))?;
    if parsed.scheme() != "https" {
        return Err(format!(
            "Media URL must use HTTPS, got '{}'",
            parsed.scheme()
        ));
    }
    let host = parsed.host_str().ok_or("URL missing host")?;
    if host.is_empty() {
        return Err("URL host is empty".to_string());
    }

    // Block internal/private addresses to prevent SSRF.
    // Handles hostname, IPv4 (including decimal/octal/hex encoding), and IPv6.
    let is_internal = is_internal_host(host);

    if is_internal {
        return Err("Internal addresses are not allowed for media URLs".to_string());
    }
    Ok(())
}

/// Check if a host string resolves to or represents an internal/private address.
/// Handles: hostnames, IPv4 literals (decimal/octal/hex), IPv6 literals, and bracketed IPv6.
fn is_internal_host(host: &str) -> bool {
    // Strip brackets from IPv6 format [::1]
    let host = host.trim_start_matches('[').trim_end_matches(']');

    // Common internal hostnames
    if host == "localhost" || host.is_empty() {
        return true;
    }

    // Try parsing as IP address — handles decimal (2130706433), octal (0177.0.0.1),
    // hex (0x7f000001), and standard IPv4/IPv6 formats.
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return is_internal_ip(&ip);
    }

    // Check raw string for private IPv4 ranges (for non-standard formats that std::net doesn't parse)
    host == "0.0.0.0"
        || host.starts_with("10.")
        || host.starts_with("192.168.")
        || (host.starts_with("172.") && {
            if let Some(second) = host.split('.').nth(1) {
                second.parse::<u8>().is_ok_and(|n| (16..=31).contains(&n))
            } else {
                false
            }
        })
        || host.starts_with("169.254.")
        || host.starts_with("127.")
}

/// Check if an IP address is internal/private/loopback.
fn is_internal_ip(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
        }
        std::net::IpAddr::V6(v6) => {
            v6.is_loopback() || v6.is_unspecified() || {
                // Check for IPv4-mapped IPv6 (::ffff:a.b.c.d)
                v6.to_ipv4()
                    .is_some_and(|v4| v4.is_loopback() || v4.is_private() || v4.is_link_local())
            }
        }
    }
}

/// Publish exactly one post (or reply) through the shared client.
///
/// Reply support is media-type agnostic: `reply_to_id` composes with
/// TEXT/IMAGE/VIDEO/CAROUSEL alike (Threads API accepts `reply_to_id` on any
/// container), matching the official publishing reference.
pub async fn publish(
    client: &ThreadsClient,
    account: &Account,
    req: &PublishRequest,
) -> Result<String> {
    validate(req)?;
    let reply_to = req.reply_to_id.as_deref();
    match req.media_type.as_str() {
        "TEXT" => {
            client
                .publish_text(
                    account,
                    req.caption.as_deref().unwrap_or(""),
                    req.location_id.as_deref(),
                    reply_to,
                )
                .await
        }
        "IMAGE" => {
            client
                .publish_image(
                    account,
                    req.caption.as_deref(),
                    req.image_url().unwrap_or_default(),
                    req.alt_text.as_deref(),
                    req.location_id.as_deref(),
                    reply_to,
                )
                .await
        }
        "VIDEO" => {
            client
                .publish_video(
                    account,
                    req.caption.as_deref(),
                    req.video_url().unwrap_or_default(),
                    req.location_id.as_deref(),
                    reply_to,
                )
                .await
        }
        "CAROUSEL" => {
            let total = req.media_urls.len();
            let mut children_ids = Vec::with_capacity(total);
            for (i, url) in req.media_urls.iter().enumerate() {
                match client
                    .create_carousel_item(account, "IMAGE", Some(url.as_str()), None, None)
                    .await
                {
                    Ok(id) => children_ids.push(id),
                    Err(e) => {
                        let idx = i + 1;
                        let ok = children_ids.len();
                        // #257: surface WHICH child failed, the offending URL,
                        // and which child containers already exist. Without
                        // this every carousel failure looks identical and
                        // production incidents (2026-09-08, 2026-09-10) require
                        // guesswork about the failure point.
                        tracing::error!(
                            "Partial carousel failure at child {idx}/{total} after {ok} ok. \
                             Orphaned children IDs (manual cleanup needed): {children_ids:?}"
                        );
                        return Err(TitenError::InvalidRequest(format!(
                            "Failed to create carousel item {idx}/{total} ({url}): {e}. \
                             {ok} child container(s) already created: {children_ids:?}"
                        )));
                    }
                }
            }
            client
                .publish_carousel(account, req.caption.as_deref(), &children_ids, reply_to)
                .await
        }
        other => Err(TitenError::InvalidRequest(format!(
            "Unsupported media type: {other}"
        ))),
    }
}

// ─── Create + publish (shared by POST /api/posts and the MCP create_post tool) ───

/// Why [`create_and_publish`] failed, in transport-neutral form.
/// `status` is the HTTP status the API maps it to; `code` is a stable machine code.
#[derive(Debug, Clone)]
pub struct PostFailure {
    pub status: u16,
    pub code: &'static str,
    pub message: String,
}

impl PostFailure {
    pub(crate) fn new(status: u16, code: &'static str, message: impl Into<String>) -> Self {
        Self { status, code, message: message.into() }
    }
}

impl std::fmt::Display for PostFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.message, self.code)
    }
}

/// Publish a post to Threads right now and record it.
///
/// Handles TEXT / IMAGE / VIDEO / CAROUSEL (incl. `media_ids` from the media
/// library), replies (`reply_to_id`), caption limits, SSRF-safe media URLs and
/// per-account rate limiting.
pub async fn create_and_publish(
    store: &crate::Store,
    client: &ThreadsClient,
    input: crate::models::CreatePost,
) -> std::result::Result<crate::models::Post, PostFailure> {
    // #136: Threads accepts at most 500 chars; keep one in reserve.
    if let Some(ref c) = input.caption {
        let n = c.chars().count();
        if n > 499 {
            return Err(PostFailure::new(
                400,
                "CAPTION_TOO_LONG",
                format!("Caption exceeds the 499-character limit (got {n}; Threads accepts at most 500)"),
            ));
        }
    }

    let account = store
        .get_account(&input.account_id)
        .await
        .map_err(|e| PostFailure::new(404, "ACCOUNT_NOT_FOUND", e.to_string()))?;

    store
        .check_rate_limit(&input.account_id, "post", 250)
        .await
        .map_err(|e| PostFailure::new(429, "RATE_LIMITED", e.to_string()))?;

    // Resolve media-library IDs → URLs (replaces image_urls; implies CAROUSEL).
    let mut input = input;
    if let Some(ids) = input.media_ids.clone().filter(|v| !v.is_empty()) {
        let mut urls = Vec::with_capacity(ids.len());
        for mid in &ids {
            let asset = store.get_media_asset(mid).await.map_err(|e| {
                PostFailure::new(400, "MEDIA_NOT_FOUND", format!("Media asset {mid} not found: {e}"))
            })?;
            let url = asset.s3_url.ok_or_else(|| {
                PostFailure::new(400, "MEDIA_NO_URL", format!("Media asset {mid} has no S3 URL"))
            })?;
            urls.push(url);
        }
        input.image_urls = Some(urls);
        if input.media_type.is_none() {
            input.media_type = Some("CAROUSEL".to_string());
        }
    }

    let media_type = input.media_type.clone().unwrap_or_else(|| "TEXT".to_string());

    // Per-type media selection so a stray field can never feed the wrong container.
    let media_urls: Vec<String> = match media_type.as_str() {
        "IMAGE" => input.image_url.clone().into_iter().collect(),
        "VIDEO" => input.video_url.clone().into_iter().collect(),
        "CAROUSEL" => input.image_urls.clone().unwrap_or_default(),
        _ => Vec::new(),
    };

    let reply_to_id = input
        .reply_to_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let req = PublishRequest {
        media_type,
        caption: input.caption.clone(),
        media_urls,
        alt_text: input.alt_text.clone(),
        location_id: None,
        reply_to_id,
    };

    // Reject bad input before touching Threads.
    if req.media_type == "TEXT" && req.caption.as_deref().map(str::trim).unwrap_or("").is_empty() {
        return Err(PostFailure::new(400, "EMPTY_POST", "TEXT posts require a non-empty caption"));
    }
    if let Err(e) = validate(&req) {
        let code = if req.media_type == "CAROUSEL" && req.media_urls.len() < 2 {
            "INVALID_CAROUSEL_COUNT"
        } else {
            "INVALID_MEDIA_URL"
        };
        let code = if matches!(e, TitenError::InvalidRequest(ref m) if m.contains("Unsupported media type")) {
            "UNSUPPORTED_MEDIA_TYPE"
        } else {
            code
        };
        return Err(PostFailure::new(400, code, e.to_string()));
    }

    let threads_post_id = publish(client, &account, &req)
        .await
        .map_err(|e| PostFailure::new(502, "THREADS_API_ERROR", e.to_string()))?;

    if let Err(e) = store.track_rate(&input.account_id, "post").await {
        tracing::warn!("Failed to track rate for post: {e}");
    }

    // Permalink is best-effort.
    let permalink = client
        .get_permalink(&account, &threads_post_id)
        .await
        .ok()
        .flatten();

    let db_id = uuid::Uuid::now_v7().to_string();
    store
        .create_post_with_threads_id(&db_id, &input, &threads_post_id, permalink.as_deref())
        .await
        .map_err(|e| PostFailure::new(500, "CREATE_FAILED", e.to_string()))
}

// ─── Schedule creation with the same rules as publishing ───

/// Create a schedule (always starts as a draft needing approval unless
/// `auto_approve`), rejecting anything the scheduler could not later publish:
/// bad `scheduled_at`, over-long caption, unknown media type, missing /
/// non-https / internal media URLs, carousels outside 2–20 items, empty reply
/// targets. Used by the MCP `schedule_post` tool.
pub async fn create_schedule_checked(
    store: &crate::Store,
    input: crate::models::CreateSchedule,
) -> std::result::Result<crate::models::Schedule, PostFailure> {
    for text in [&input.caption, &input.text_attachment].into_iter().flatten() {
        let n = text.chars().count();
        if n > 499 {
            return Err(PostFailure::new(
                400,
                "CAPTION_TOO_LONG",
                format!("Caption exceeds the 499-character limit (got {n}; Threads accepts at most 500)"),
            ));
        }
    }
    if crate::time::parse_utc(&input.scheduled_at).is_none() {
        return Err(PostFailure::new(
            400,
            "INVALID_SCHEDULED_AT",
            format!(
                "scheduled_at must be an ISO 8601 / RFC 3339 timestamp with offset, got: {}",
                input.scheduled_at
            ),
        ));
    }
    if let Some(r) = input.reply_to_id.as_deref() {
        if r.trim().is_empty() {
            return Err(PostFailure::new(
                400,
                "INVALID_REPLY_TO_ID",
                "reply_to_id must be a non-empty Threads post ID",
            ));
        }
    }
    store
        .get_account(&input.account_id)
        .await
        .map_err(|e| PostFailure::new(404, "ACCOUNT_NOT_FOUND", e.to_string()))?;

    let media_type = input.media_type.clone().unwrap_or_else(|| "TEXT".to_string());
    let req = PublishRequest {
        media_type: media_type.clone(),
        caption: input.caption.clone(),
        media_urls: input.media_urls.clone().unwrap_or_default(),
        ..Default::default()
    };
    if media_type == "TEXT" && req.caption.as_deref().map(str::trim).unwrap_or("").is_empty() {
        return Err(PostFailure::new(400, "EMPTY_POST", "TEXT posts require a non-empty caption"));
    }
    if let Err(e) = validate(&req) {
        let code = match &e {
            TitenError::InvalidRequest(m) if m.contains("Unsupported media type") => {
                "UNSUPPORTED_MEDIA_TYPE"
            }
            _ if media_type == "CAROUSEL" && !(2..=20).contains(&req.media_urls.len()) => {
                "INVALID_CAROUSEL_COUNT"
            }
            _ => "INVALID_MEDIA_URL",
        };
        return Err(PostFailure::new(400, code, e.to_string()));
    }

    let id = uuid::Uuid::now_v7().to_string();
    store
        .create_schedule(&id, &input)
        .await
        .map_err(|e| PostFailure::new(500, "CREATE_FAILED", e.to_string()))
}
