// SPDX-License-Identifier: MPL-2.0
//! First-party bounded stdin-to-DASObjectStore completion helper.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{self, BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

const PROTOCOL_SCHEMA: &str = "pinakotheke.object-ingest-stream.v1";
const CONFIG_SCHEMA: &str = "pinakotheke.das-stream-ingest-helper.v1";
const DEFAULT_MAX_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_HEADER_BYTES: u64 = 16 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Header {
    schema_version: String,
    ingest_id: String,
    endpoint_id: String,
    object_store_id: String,
    object_key: String,
    expected_size_bytes: u64,
    expected_checksum: String,
    content_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    schema_version: String,
    endpoint_id: String,
    #[serde(default)]
    dasobjectstore_remote_executable: Option<PathBuf>,
    #[serde(default)]
    dasobjectstore_remote_config: Option<PathBuf>,
    #[serde(default)]
    daemon_socket: Option<PathBuf>,
    #[serde(default)]
    container_execution: Option<ContainerExecution>,
    max_object_bytes: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContainerExecution {
    docker_executable: PathBuf,
    compose_file: PathBuf,
    managed_scratch_root: PathBuf,
    container_scratch_root: PathBuf,
    remote_config: PathBuf,
    aws_credentials: PathBuf,
    service: String,
    daemon_socket: PathBuf,
}

#[derive(Debug, Serialize)]
struct Receipt {
    schema_version: &'static str,
    endpoint_id: String,
    object_store_id: String,
    object_key: String,
    size_bytes: u64,
    checksum: String,
    object_reference: String,
}

pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config_path = std::env::var_os("PINAKOTHEKE_DAS_STREAM_HELPER_CONFIG")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(|home| PathBuf::from(home).join(".x-img/config/das-stream-ingest-helper.json"))
        })
        .ok_or("PINAKOTHEKE_DAS_STREAM_HELPER_CONFIG or HOME is required")?;
    let config = load_config(&config_path)?;
    let mut input = BufReader::new(io::stdin().lock());
    let receipt = ingest(&mut input, &config)?;
    serde_json::to_writer(io::stdout().lock(), &receipt)?;
    println!();
    Ok(())
}

fn ingest<R: BufRead>(
    input: &mut R,
    config: &Config,
) -> Result<Receipt, Box<dyn std::error::Error>> {
    let mut header_bytes = Vec::new();
    input
        .by_ref()
        .take(MAX_HEADER_BYTES + 1)
        .read_until(b'\n', &mut header_bytes)?;
    if header_bytes.is_empty()
        || header_bytes.len() as u64 > MAX_HEADER_BYTES
        || header_bytes.last() != Some(&b'\n')
    {
        return Err("stream ingest header is missing or exceeds 16 KiB".into());
    }
    header_bytes.pop();
    let header: Header = serde_json::from_slice(&header_bytes)?;
    validate_header(&header, config)?;

    let scratch = match &config.container_execution {
        Some(container) => Scratch::create_in(&container.managed_scratch_root)?,
        None => Scratch::create()?,
    };
    let payload = scratch.path.join("payload");
    let mut file = File::create(&payload)?;
    let mut hasher = Sha256::new();
    let mut remaining = header.expected_size_bytes;
    let mut buffer = [0_u8; 64 * 1024];
    while remaining > 0 {
        let limit = usize::try_from(remaining.min(buffer.len() as u64))?;
        let count = input.read(&mut buffer[..limit])?;
        if count == 0 {
            return Err("stream ingest payload ended before its declared length".into());
        }
        file.write_all(&buffer[..count])?;
        hasher.update(&buffer[..count]);
        remaining -= count as u64;
    }
    file.sync_all()?;
    drop(file);
    let mut trailing = [0_u8; 1];
    if input.read(&mut trailing)? != 0 {
        return Err("stream ingest payload exceeded its declared length".into());
    }
    let checksum = format!("sha256:{:x}", hasher.finalize());
    if checksum != header.expected_checksum {
        return Err("stream ingest payload checksum did not match its plan".into());
    }

    let mut upload = upload_command(config, &scratch, &payload, &header)?;
    let (status, stdout) = output_bounded(&mut upload, 64 * 1024)?;
    if !status.success() {
        return Err("DASObjectStore remote upload failed".into());
    }
    let report = String::from_utf8(stdout)?;
    if !report.lines().any(|line| {
        line.starts_with("Final:")
            && line.contains("state=Complete")
            && line.contains("stage=remote_s3_transfer_complete")
    }) {
        return Err("DASObjectStore did not report verified completion".into());
    }
    Ok(Receipt {
        schema_version: PROTOCOL_SCHEMA,
        endpoint_id: header.endpoint_id.clone(),
        object_store_id: header.object_store_id.clone(),
        object_key: header.object_key.clone(),
        size_bytes: header.expected_size_bytes,
        checksum,
        object_reference: format!(
            "dasobjectstore:{}:{}:{}",
            header.endpoint_id, header.object_store_id, header.object_key
        ),
    })
}

fn load_config(path: &Path) -> Result<Config, Box<dyn std::error::Error>> {
    require_private_regular(path)?;
    let bytes = fs::read(path)?;
    if bytes.len() > 16 * 1024 {
        return Err("DAS stream helper config exceeds 16 KiB".into());
    }
    let config: Config = serde_json::from_slice(&bytes)?;
    let max = config.max_object_bytes.unwrap_or(DEFAULT_MAX_BYTES);
    if config.schema_version != CONFIG_SCHEMA
        || !identifier(&config.endpoint_id)
        || max == 0
        || max > 64 * 1024 * 1024 * 1024
    {
        return Err("DAS stream helper config is invalid".into());
    }
    match &config.container_execution {
        Some(container) => {
            if config.dasobjectstore_remote_executable.is_some()
                || config.dasobjectstore_remote_config.is_some()
                || config.daemon_socket.is_some()
            {
                return Err("native and container DAS execution cannot be combined".into());
            }
            validate_container_execution(container)?;
        }
        None => {
            require_executable(
                config
                    .dasobjectstore_remote_executable
                    .as_deref()
                    .ok_or("native DAS remote executable is required")?,
            )?;
            require_private_regular(
                config
                    .dasobjectstore_remote_config
                    .as_deref()
                    .ok_or("native DAS remote config is required")?,
            )?;
            if !config
                .daemon_socket
                .as_deref()
                .is_some_and(Path::is_absolute)
            {
                return Err("native DAS daemon socket must be absolute".into());
            }
        }
    }
    Ok(config)
}

fn validate_header(header: &Header, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let max = config.max_object_bytes.unwrap_or(DEFAULT_MAX_BYTES);
    if header.schema_version != PROTOCOL_SCHEMA
        || header.endpoint_id != config.endpoint_id
        || !identifier(&header.ingest_id)
        || !identifier(&header.endpoint_id)
        || !identifier(&header.object_store_id)
        || !safe_object_key(&header.object_key)
        || header.expected_size_bytes == 0
        || header.expected_size_bytes > max
        || !checksum(&header.expected_checksum)
        || !matches!(
            header.content_type.as_str(),
            "video/mp4" | "video/webm" | "image/webp" | "application/json"
        )
    {
        return Err("stream ingest request is invalid or changed authority".into());
    }
    Ok(())
}

fn validate_container_execution(
    container: &ContainerExecution,
) -> Result<(), Box<dyn std::error::Error>> {
    require_executable(&container.docker_executable)?;
    require_private_regular(&container.compose_file)?;
    require_private_regular(&container.remote_config)?;
    require_private_regular(&container.aws_credentials)?;
    if container.service != "dasobjectstored"
        || !container.managed_scratch_root.is_absolute()
        || !container.container_scratch_root.is_absolute()
        || !container.daemon_socket.is_absolute()
        || container.managed_scratch_root == Path::new("/")
        || container.container_scratch_root == Path::new("/")
    {
        return Err("container execution boundary is invalid".into());
    }
    let root = fs::canonicalize(&container.managed_scratch_root)?;
    if root != container.managed_scratch_root
        || fs::symlink_metadata(&root)?.file_type().is_symlink()
    {
        return Err("managed scratch root must be a canonical directory".into());
    }
    Ok(())
}

fn upload_command(
    config: &Config,
    scratch: &Scratch,
    payload: &Path,
    header: &Header,
) -> Result<Command, Box<dyn std::error::Error>> {
    if let Some(container) = &config.container_execution {
        let remote_config = scratch.path.join("remote.json");
        let credentials = scratch.path.join("aws-credentials");
        copy_private(&container.remote_config, &remote_config)?;
        copy_private(&container.aws_credentials, &credentials)?;
        let container_job = translated_container_path(container, &scratch.path)?;
        let mut command = Command::new(&container.docker_executable);
        command
            .args(["compose", "-f"])
            .arg(&container.compose_file)
            .args(["exec", "-T", "-e"])
            .arg(format!(
                "AWS_SHARED_CREDENTIALS_FILE={}",
                container_job.join("aws-credentials").display()
            ))
            .arg(&container.service)
            .arg("dasobjectstore-remote")
            .arg("--config")
            .arg(container_job.join("remote.json"))
            .arg("upload")
            .arg(&header.object_store_id)
            .arg("--source")
            .arg(container_job.join("payload"))
            .arg("--key")
            .arg(&header.object_key)
            .arg("--content-type")
            .arg(&header.content_type)
            .args(["--no-progress", "--submit-to-daemon", "--daemon-socket"])
            .arg(&container.daemon_socket)
            .stderr(Stdio::null());
        return Ok(command);
    }
    let mut command = Command::new(
        config
            .dasobjectstore_remote_executable
            .as_deref()
            .ok_or("native DAS remote executable is required")?,
    );
    command
        .arg("--config")
        .arg(
            config
                .dasobjectstore_remote_config
                .as_deref()
                .ok_or("native DAS remote config is required")?,
        )
        .arg("upload")
        .arg(&header.object_store_id)
        .arg("--source")
        .arg(payload)
        .arg("--key")
        .arg(&header.object_key)
        .arg("--content-type")
        .arg(&header.content_type)
        .args(["--no-progress", "--submit-to-daemon", "--daemon-socket"])
        .arg(
            config
                .daemon_socket
                .as_deref()
                .ok_or("native DAS daemon socket is required")?,
        )
        .stderr(Stdio::null());
    Ok(command)
}

fn translated_container_path(
    container: &ContainerExecution,
    host_path: &Path,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let relative = host_path
        .strip_prefix(&container.managed_scratch_root)
        .map_err(|_| "scratch path escaped the managed root")?;
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err("scratch path is not a direct managed descendant".into());
    }
    Ok(container.container_scratch_root.join(relative))
}

fn copy_private(source: &Path, destination: &Path) -> io::Result<()> {
    require_private_regular(source)?;
    fs::copy(source, destination)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(destination, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn output_bounded(command: &mut Command, limit: u64) -> io::Result<(ExitStatus, Vec<u8>)> {
    let mut child = command.stdout(Stdio::piped()).spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("child stdout unavailable"))?;
    let mut bytes = Vec::new();
    stdout.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        let _ = child.kill();
        let _ = child.wait();
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "child output exceeded the bounded protocol",
        ));
    }
    Ok((child.wait()?, bytes))
}

fn require_private_regular(path: &Path) -> io::Result<()> {
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path must be absolute",
        ));
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path must be a regular file",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "file must be mode 0600 or stricter",
            ));
        }
    }
    Ok(())
}

fn require_executable(path: &Path) -> io::Result<()> {
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "executable must be absolute",
        ));
    }
    let metadata = fs::symlink_metadata(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.permissions().mode() & 0o111 == 0
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "executable is not reviewed",
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

fn safe_object_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && !value.starts_with('/')
        && !value.contains("//")
        && value
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

fn checksum(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn create() -> io::Result<Self> {
        Self::create_in(&std::env::temp_dir())
    }

    fn create_in(root: &Path) -> io::Result<Self> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_nanos();
        let path = root.join(format!(
            ".pinakotheke-stream-ingest-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
        }
        Ok(Self { path })
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{io::Cursor, os::unix::fs::PermissionsExt};

    #[test]
    fn streams_exact_payload_through_verified_daemon_completion() {
        let root = std::env::temp_dir().join(format!(
            "pinakotheke-stream-helper-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir(&root).unwrap();
        let remote = root.join("remote");
        fs::write(
            &remote,
            "#!/bin/sh\nprintf '%s' \"$*\" | grep -q -- 'upload store-1 --source .* --key video/fixture.mp4 --content-type video/mp4 --no-progress --submit-to-daemon --daemon-socket' || exit 9\nprintf 'Final: job state=Complete stage=remote_s3_transfer_complete\\n'\n",
        )
        .unwrap();
        fs::set_permissions(&remote, fs::Permissions::from_mode(0o700)).unwrap();
        let remote_config = root.join("remote.json");
        fs::write(&remote_config, "{}").unwrap();
        fs::set_permissions(&remote_config, fs::Permissions::from_mode(0o600)).unwrap();
        let config = Config {
            schema_version: CONFIG_SCHEMA.into(),
            endpoint_id: "endpoint-1".into(),
            dasobjectstore_remote_executable: Some(remote),
            dasobjectstore_remote_config: Some(remote_config),
            daemon_socket: Some(root.join("daemon.sock")),
            container_execution: None,
            max_object_bytes: Some(1024),
        };
        let payload = b"normalized fixture";
        let digest = format!("sha256:{:x}", Sha256::digest(payload));
        let header = format!(
            "{{\"schema_version\":\"{PROTOCOL_SCHEMA}\",\"ingest_id\":\"job-1:normalized\",\"endpoint_id\":\"endpoint-1\",\"object_store_id\":\"store-1\",\"object_key\":\"video/fixture.mp4\",\"expected_size_bytes\":{},\"expected_checksum\":\"{digest}\",\"content_type\":\"video/mp4\"}}\n",
            payload.len()
        );
        let mut bytes = header.into_bytes();
        bytes.extend(payload);
        let receipt = ingest(&mut Cursor::new(bytes), &config).unwrap();
        assert_eq!(receipt.size_bytes, payload.len() as u64);
        assert_eq!(receipt.checksum, digest);
        assert_eq!(
            receipt.object_reference,
            "dasobjectstore:endpoint-1:store-1:video/fixture.mp4"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_trailing_bytes_before_any_authority_command() {
        let config = Config {
            schema_version: CONFIG_SCHEMA.into(),
            endpoint_id: "endpoint-1".into(),
            dasobjectstore_remote_executable: Some(PathBuf::from("/does/not/run")),
            dasobjectstore_remote_config: Some(PathBuf::from("/does/not/read")),
            daemon_socket: Some(PathBuf::from("/does/not/connect")),
            container_execution: None,
            max_object_bytes: Some(1024),
        };
        let payload = b"fixture";
        let digest = format!("sha256:{:x}", Sha256::digest(payload));
        let bytes = format!(
            "{{\"schema_version\":\"{PROTOCOL_SCHEMA}\",\"ingest_id\":\"job-1:poster\",\"endpoint_id\":\"endpoint-1\",\"object_store_id\":\"store-1\",\"object_key\":\"video/poster.webp\",\"expected_size_bytes\":{},\"expected_checksum\":\"{digest}\",\"content_type\":\"image/webp\"}}\n{}x",
            payload.len(),
            String::from_utf8_lossy(payload)
        );
        assert!(ingest(&mut Cursor::new(bytes.into_bytes()), &config).is_err());
    }
}
