// SPDX-License-Identifier: MPL-2.0
//! Process-isolated host adapter for live capture-destination revalidation.

use serde::{Deserialize, Serialize};
use std::{
    io::{self, BufRead, BufReader, Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use x_img_api::{CaptureDestinationAuthorityState, HostCaptureDestinationRevalidateBackend};
use x_img_core::viewed_media::{CaptureDestinationSnapshot, CaptureKind};

const SCHEMA: &str = "pinakotheke.destination-revalidate-helper.v1";
const RESPONSE_LIMIT: u64 = 16 * 1024;
const TIMEOUT: Duration = Duration::from_secs(10);
const MAX_AGE_SECONDS: u64 = 30;
const MAX_FUTURE_SKEW_SECONDS: u64 = 5;

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct Request<'a> {
    schema_version: &'static str,
    actor_id: &'a str,
    endpoint_id: &'a str,
    object_store_id: &'a str,
    selection_revision: u64,
    object_type: &'static str,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
enum Response {
    Ready {
        schema_version: String,
        endpoint_id: String,
        object_store_id: String,
        selection_revision: u64,
        object_type: String,
        checked_at_epoch_seconds: u64,
        endpoint_present: bool,
        object_store_present: bool,
        tls_trusted: bool,
        paired: bool,
        pairing_expires_at_epoch_seconds: u64,
        ready: bool,
        writable: bool,
        quota_available_bytes: u64,
    },
    RemovedEndpoint {
        schema_version: String,
    },
    RemovedStore {
        schema_version: String,
    },
    TlsUntrusted {
        schema_version: String,
    },
    PairingExpired {
        schema_version: String,
    },
    NeedsReconnect {
        schema_version: String,
    },
    ReadOnly {
        schema_version: String,
    },
    OverQuota {
        schema_version: String,
    },
    UnsupportedObjectType {
        schema_version: String,
    },
    Unauthorized {
        schema_version: String,
    },
    Unavailable {
        schema_version: String,
    },
}

pub(crate) fn backend(helper: &Path) -> io::Result<HostCaptureDestinationRevalidateBackend> {
    validate_helper(helper)?;
    let helper = helper.to_owned();
    Ok(HostCaptureDestinationRevalidateBackend::new(Box::new(
        move |actor_id, snapshot, capture_kind| {
            revalidate(&helper, actor_id, snapshot, capture_kind).map_err(|error| error.to_string())
        },
    )))
}

fn revalidate(
    helper: &Path,
    actor_id: &str,
    snapshot: &CaptureDestinationSnapshot,
    capture_kind: CaptureKind,
) -> io::Result<CaptureDestinationAuthorityState> {
    validate_helper(helper)?;
    if !identifier(actor_id) || !snapshot.is_valid() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "destination revalidation identity is invalid",
        ));
    }
    let object_type = match capture_kind {
        CaptureKind::ExplicitVideo => "video",
        CaptureKind::ObservedThumbnail | CaptureKind::ExplicitOriginal => "image",
    };
    let mut child = Command::new(helper)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    let request = Request {
        schema_version: SCHEMA,
        actor_id,
        endpoint_id: &snapshot.endpoint_id,
        object_store_id: &snapshot.object_store_id,
        selection_revision: snapshot.selection_revision,
        object_type,
    };
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("destination revalidation helper stdin unavailable"))?;
    serde_json::to_writer(&mut stdin, &request).map_err(io::Error::other)?;
    stdin.write_all(b"\n")?;
    drop(stdin);
    wait_bounded(&mut child)?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("destination revalidation helper response unavailable"))?;
    let mut reader = BufReader::new(stderr).take(RESPONSE_LIMIT + 1);
    let mut response = String::new();
    reader.read_line(&mut response)?;
    let mut trailing = String::new();
    reader.read_to_string(&mut trailing)?;
    if response.len() as u64 > RESPONSE_LIMIT || !response.ends_with('\n') || !trailing.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "destination revalidation response is missing, oversized, or not one line",
        ));
    }
    let response: Response = serde_json::from_str(&response).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "destination revalidation response is invalid",
        )
    })?;
    validate_response(response, snapshot, object_type)
}

fn validate_response(
    response: Response,
    snapshot: &CaptureDestinationSnapshot,
    object_type: &str,
) -> io::Result<CaptureDestinationAuthorityState> {
    let schema = match &response {
        Response::Ready { schema_version, .. }
        | Response::RemovedEndpoint { schema_version }
        | Response::RemovedStore { schema_version }
        | Response::TlsUntrusted { schema_version }
        | Response::PairingExpired { schema_version }
        | Response::NeedsReconnect { schema_version }
        | Response::ReadOnly { schema_version }
        | Response::OverQuota { schema_version }
        | Response::UnsupportedObjectType { schema_version }
        | Response::Unauthorized { schema_version }
        | Response::Unavailable { schema_version } => schema_version,
    };
    if schema != SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "destination revalidation schema is unsupported",
        ));
    }
    if let Some((kind, message)) = categorical_failure(&response) {
        return Err(io::Error::new(kind, message));
    }
    let Response::Ready {
        endpoint_id,
        object_store_id,
        selection_revision,
        object_type: actual_object_type,
        checked_at_epoch_seconds,
        endpoint_present,
        object_store_present,
        tls_trusted,
        paired,
        pairing_expires_at_epoch_seconds,
        ready,
        writable,
        quota_available_bytes,
        ..
    } = response
    else {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "destination authority rejected the reviewed destination",
        ));
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_secs();
    if endpoint_id != snapshot.endpoint_id
        || object_store_id != snapshot.object_store_id
        || selection_revision != snapshot.selection_revision
        || actual_object_type != object_type
        || checked_at_epoch_seconds > now.saturating_add(MAX_FUTURE_SKEW_SECONDS)
        || now.saturating_sub(checked_at_epoch_seconds) > MAX_AGE_SECONDS
        || !endpoint_present
        || !object_store_present
        || !tls_trusted
        || !paired
        || pairing_expires_at_epoch_seconds <= now
        || !ready
        || !writable
        || quota_available_bytes == 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "destination authority did not confirm the exact current writable destination",
        ));
    }
    Ok(CaptureDestinationAuthorityState {
        endpoint_id,
        object_store_id,
        endpoint_present,
        object_store_present,
        tls_trusted,
        paired,
        pairing_expires_at_epoch_seconds,
        ready,
        writable,
        quota_available_bytes,
    })
}

fn categorical_failure(response: &Response) -> Option<(io::ErrorKind, &'static str)> {
    match response {
        Response::Ready { .. } => None,
        Response::RemovedEndpoint { .. } => Some((
            io::ErrorKind::NotFound,
            "destination authority reports removed endpoint",
        )),
        Response::RemovedStore { .. } => Some((
            io::ErrorKind::NotFound,
            "destination authority reports removed ObjectStore",
        )),
        Response::TlsUntrusted { .. } => Some((
            io::ErrorKind::PermissionDenied,
            "destination authority reports untrusted TLS",
        )),
        Response::PairingExpired { .. } => Some((
            io::ErrorKind::PermissionDenied,
            "destination authority reports expired pairing",
        )),
        Response::NeedsReconnect { .. } => Some((
            io::ErrorKind::WouldBlock,
            "destination authority requires reconnect",
        )),
        Response::ReadOnly { .. } => Some((
            io::ErrorKind::PermissionDenied,
            "destination authority reports read-only ObjectStore",
        )),
        Response::OverQuota { .. } => Some((
            io::ErrorKind::PermissionDenied,
            "destination authority reports exhausted quota",
        )),
        Response::UnsupportedObjectType { .. } => Some((
            io::ErrorKind::InvalidInput,
            "destination authority rejects the media object type",
        )),
        Response::Unauthorized { .. } => Some((
            io::ErrorKind::PermissionDenied,
            "destination authority rejects the actor",
        )),
        Response::Unavailable { .. } => Some((
            io::ErrorKind::WouldBlock,
            "destination authority is unavailable",
        )),
    }
}

fn wait_bounded(child: &mut Child) -> io::Result<()> {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if let Some(status) = child.try_wait()? {
            return if status.success() {
                Ok(())
            } else {
                Err(io::Error::other("destination revalidation helper failed"))
            };
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "destination revalidation helper timed out",
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn validate_helper(path: &Path) -> io::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !path.is_absolute() || metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "destination revalidation helper must be an absolute regular file",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "destination revalidation helper must be executable",
            ));
        }
    }
    Ok(())
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    static SCRIPT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn snapshot() -> CaptureDestinationSnapshot {
        CaptureDestinationSnapshot {
            endpoint_id: "endpoint-1".into(),
            object_store_id: "store-1".into(),
            selection_revision: 7,
        }
    }

    fn response(overrides: &str) -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        format!(
            "{{\"outcome\":\"ready\",\"schema_version\":\"{SCHEMA}\",\"endpoint_id\":\"endpoint-1\",\"object_store_id\":\"store-1\",\"selection_revision\":7,\"object_type\":\"image\",\"checked_at_epoch_seconds\":{now},\"endpoint_present\":true,\"object_store_present\":true,\"tls_trusted\":true,\"paired\":true,\"pairing_expires_at_epoch_seconds\":{},\"ready\":true,\"writable\":true,\"quota_available_bytes\":1{overrides}}}",
            now + 60
        )
    }

    fn script(body: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "pinakotheke-destination-revalidate-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SCRIPT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path
    }

    #[test]
    fn accepts_only_fresh_exact_live_authority() {
        let payload = response("");
        let helper = script(&format!("printf '%s\\n' '{payload}' >&2"));
        let state = revalidate(
            &helper,
            "actor-1",
            &snapshot(),
            CaptureKind::ObservedThumbnail,
        )
        .expect("live exact authority");
        assert_eq!(state.endpoint_id, "endpoint-1");
        fs::remove_file(helper).unwrap();
    }

    #[test]
    fn rejects_changed_revision_object_type_and_unknown_fields() {
        for payload in [
            response(",\"unexpected\":true"),
            response("").replace("\"selection_revision\":7", "\"selection_revision\":8"),
            response("").replace("\"object_type\":\"image\"", "\"object_type\":\"video\""),
        ] {
            let helper = script(&format!("printf '%s\\n' '{payload}' >&2"));
            assert!(
                revalidate(
                    &helper,
                    "actor-1",
                    &snapshot(),
                    CaptureKind::ObservedThumbnail
                )
                .is_err()
            );
            fs::remove_file(helper).unwrap();
        }
    }

    #[test]
    fn rejects_categorical_failure_without_exposing_helper_details() {
        let helper = script(&format!(
            "printf '%s\\n' '{{\"outcome\":\"over_quota\",\"schema_version\":\"{SCHEMA}\"}}' >&2"
        ));
        let error = revalidate(
            &helper,
            "actor-1",
            &snapshot(),
            CaptureKind::ObservedThumbnail,
        )
        .expect_err("quota blocks");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(error.to_string().contains("quota"));
        fs::remove_file(helper).unwrap();
    }

    #[test]
    fn rejects_stale_expired_unwritable_and_empty_quota_ready_claims() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        for payload in [
            response("").replace(
                &format!("\"checked_at_epoch_seconds\":{now}"),
                &format!("\"checked_at_epoch_seconds\":{}", now - 31),
            ),
            response("").replace(
                &format!("\"pairing_expires_at_epoch_seconds\":{}", now + 60),
                &format!("\"pairing_expires_at_epoch_seconds\":{now}"),
            ),
            response("").replace("\"writable\":true", "\"writable\":false"),
            response("").replace("\"quota_available_bytes\":1", "\"quota_available_bytes\":0"),
        ] {
            let helper = script(&format!("printf '%s\\n' '{payload}' >&2"));
            assert!(
                revalidate(
                    &helper,
                    "actor-1",
                    &snapshot(),
                    CaptureKind::ObservedThumbnail
                )
                .is_err()
            );
            fs::remove_file(helper).unwrap();
        }
    }

    #[test]
    fn unavailable_is_a_retryable_bounded_category() {
        let helper = script(&format!(
            "printf '%s\\n' '{{\"outcome\":\"unavailable\",\"schema_version\":\"{SCHEMA}\"}}' >&2"
        ));
        let error = revalidate(
            &helper,
            "actor-1",
            &snapshot(),
            CaptureKind::ObservedThumbnail,
        )
        .expect_err("unavailable blocks");
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        assert_eq!(error.to_string(), "destination authority is unavailable");
        fs::remove_file(helper).unwrap();
    }

    #[test]
    fn rejects_symlinked_helper() {
        let target = script("exit 0");
        let link = target.with_extension("link");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(validate_helper(&link).is_err());
        fs::remove_file(link).unwrap();
        fs::remove_file(target).unwrap();
    }
}
