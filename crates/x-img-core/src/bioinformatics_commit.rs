// SPDX-License-Identifier: MPL-2.0
//! Confirmed, bounded direct commit of one bioinformatics plan file.
//!
//! The caller supplies a streaming iterator. This module never creates a local
//! payload file or buffers a file; it records only bounded verified provenance.

#![allow(missing_docs)]

use std::collections::BTreeMap;

use crate::{
    bioinformatics::{PolicyResult, ResourceFile, TransferPlan},
    destination_selection::{
        DestinationAuthorityState, DestinationRevalidationError, DestinationSnapshot,
        revalidate_before_commit,
    },
    object_ingest::{
        BeginIngest, CommitReceipt, ObjectIngestBackend, ObjectIngestError, StreamingObjectIngestor,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BioinformaticsProvenance {
    pub authority: String,
    pub accession_or_url: String,
    pub release: String,
    pub file_id: String,
    pub filename: String,
    pub source_checksum: String,
    pub transport: String,
    pub rights_note: String,
    pub endpoint_id: String,
    pub object_store_id: String,
    pub object_key: String,
    pub object_reference: String,
    pub committed_at_unix_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BioinformaticsCommitError {
    Unconfirmed,
    PolicyBlocked,
    UnknownFile,
    ChecksumNotSha256,
    DestinationMismatch,
    Revalidation(DestinationRevalidationError),
    Ingest(ObjectIngestError),
    DuplicateConflict,
}

impl std::fmt::Display for BioinformaticsCommitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unconfirmed => f.write_str("bioinformatics plan requires explicit confirmation"),
            Self::PolicyBlocked => f.write_str("bioinformatics plan policy does not allow commit"),
            Self::UnknownFile => {
                f.write_str("selected bioinformatics file is absent from the confirmed plan")
            }
            Self::ChecksumNotSha256 => {
                f.write_str("confirmed file must carry a SHA-256 checksum for direct commit")
            }
            Self::DestinationMismatch => {
                f.write_str("confirmed plan destination differs from the reviewed destination")
            }
            Self::Revalidation(error) => write!(f, "destination revalidation failed: {error}"),
            Self::Ingest(error) => write!(f, "direct DASObjectStore ingest failed: {error}"),
            Self::DuplicateConflict => f.write_str(
                "bioinformatics identity and checksum were previously committed differently",
            ),
        }
    }
}

impl std::error::Error for BioinformaticsCommitError {}

pub struct ConfirmedBioinformaticsCommitter<B: ObjectIngestBackend> {
    ingestor: StreamingObjectIngestor<B>,
    committed: BTreeMap<String, BioinformaticsProvenance>,
}

impl<B: ObjectIngestBackend> ConfirmedBioinformaticsCommitter<B> {
    pub fn new(backend: B) -> Self {
        Self {
            ingestor: StreamingObjectIngestor::new(backend),
            committed: BTreeMap::new(),
        }
    }

    /// Streams an explicit selected file directly to the authority. `chunks`
    /// are consumed once and are never retained after each backend write.
    #[allow(clippy::too_many_arguments)]
    pub fn commit_file<'a, I>(
        &mut self,
        plan: &TransferPlan,
        snapshot: &DestinationSnapshot,
        current: &DestinationAuthorityState,
        now_unix_seconds: u64,
        file_id: &str,
        ingest_id: String,
        max_chunk_bytes: usize,
        chunks: I,
    ) -> Result<BioinformaticsProvenance, BioinformaticsCommitError>
    where
        I: IntoIterator<Item = &'a [u8]>,
    {
        if !plan.confirmed {
            return Err(BioinformaticsCommitError::Unconfirmed);
        }
        if plan.policy != PolicyResult::Allowed {
            return Err(BioinformaticsCommitError::PolicyBlocked);
        }
        if plan.destination.endpoint_id != snapshot.endpoint_id
            || plan.destination.object_store_id != snapshot.object_store_id
        {
            return Err(BioinformaticsCommitError::DestinationMismatch);
        }
        revalidate_before_commit(snapshot, current, now_unix_seconds)
            .map_err(BioinformaticsCommitError::Revalidation)?;
        let file = plan
            .files
            .iter()
            .find(|file| file.file_id == file_id)
            .ok_or(BioinformaticsCommitError::UnknownFile)?;
        let checksum = sha256_checksum(file)?;
        let key = format!(
            "{}:{}:{}:{}",
            authority_name(plan),
            plan.accession_or_url,
            file.file_id,
            checksum
        );
        if let Some(existing) = self.committed.get(&key) {
            if existing.endpoint_id == snapshot.endpoint_id
                && existing.object_store_id == snapshot.object_store_id
            {
                return Ok(existing.clone());
            }
            return Err(BioinformaticsCommitError::DuplicateConflict);
        }
        let object_key = format!("{}{}", plan.destination.prefix, file.filename);
        let request = crate::object_ingest::IngestRequest {
            ingest_id,
            endpoint_id: snapshot.endpoint_id.clone(),
            object_store_id: snapshot.object_store_id.clone(),
            object_key: object_key.clone(),
            expected_size_bytes: file.bytes,
            expected_checksum: checksum.clone(),
            max_chunk_bytes,
        };
        let receipt = match self
            .ingestor
            .begin(request)
            .map_err(BioinformaticsCommitError::Ingest)?
        {
            BeginIngest::AlreadyCommitted(receipt) => receipt,
            BeginIngest::Active(mut session) => {
                for chunk in chunks {
                    self.ingestor
                        .write_chunk(&mut session, chunk)
                        .map_err(BioinformaticsCommitError::Ingest)?;
                }
                self.ingestor
                    .finish(session)
                    .map_err(BioinformaticsCommitError::Ingest)?
            }
        };
        let provenance = provenance(plan, file, checksum, receipt, now_unix_seconds);
        self.committed.insert(key, provenance.clone());
        Ok(provenance)
    }

    pub fn into_backend(self) -> B {
        self.ingestor.into_backend()
    }
}

fn sha256_checksum(file: &ResourceFile) -> Result<String, BioinformaticsCommitError> {
    if file.checksum.len() != 64 {
        return Err(BioinformaticsCommitError::ChecksumNotSha256);
    }
    Ok(format!("sha256:{}", file.checksum))
}
fn authority_name(plan: &TransferPlan) -> &'static str {
    match plan.authority {
        crate::bioinformatics::ResourceAuthority::Geo => "geo",
        crate::bioinformatics::ResourceAuthority::Sra => "sra",
        crate::bioinformatics::ResourceAuthority::Ena => "ena",
        crate::bioinformatics::ResourceAuthority::Ncbi => "ncbi",
    }
}
fn transport_name(file: &ResourceFile) -> &'static str {
    match file.transport {
        crate::bioinformatics::Transport::Https => "https",
        crate::bioinformatics::Transport::Ftp => "ftp",
        crate::bioinformatics::Transport::Aspera => "aspera",
    }
}
fn provenance(
    plan: &TransferPlan,
    file: &ResourceFile,
    checksum: String,
    receipt: CommitReceipt,
    committed_at_unix_seconds: u64,
) -> BioinformaticsProvenance {
    BioinformaticsProvenance {
        authority: authority_name(plan).to_owned(),
        accession_or_url: plan.accession_or_url.clone(),
        release: plan.release.clone(),
        file_id: file.file_id.clone(),
        filename: file.filename.clone(),
        source_checksum: checksum,
        transport: transport_name(file).to_owned(),
        rights_note: plan.rights_note.clone(),
        endpoint_id: receipt.endpoint_id,
        object_store_id: receipt.object_store_id,
        object_key: receipt.object_key,
        object_reference: receipt.object_reference,
        committed_at_unix_seconds,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bioinformatics::{Destination, PolicyResult, ResourceAuthority, Transport};
    use crate::object_ingest::{IngestBackendError, IngestRequest};

    #[derive(Default)]
    struct Backend {
        chunks: usize,
    }
    impl ObjectIngestBackend for Backend {
        type Upload = IngestRequest;
        fn begin(&mut self, request: &IngestRequest) -> Result<Self::Upload, IngestBackendError> {
            Ok(request.clone())
        }
        fn write_chunk(
            &mut self,
            _: &mut Self::Upload,
            _: &[u8],
        ) -> Result<(), IngestBackendError> {
            self.chunks += 1;
            Ok(())
        }
        fn complete(
            &mut self,
            upload: Self::Upload,
            expected: &CommitReceipt,
        ) -> Result<CommitReceipt, IngestBackendError> {
            assert_eq!(upload.endpoint_id, expected.endpoint_id);
            Ok(expected.clone())
        }
    }
    fn plan() -> TransferPlan {
        let mut plan = TransferPlan::new(
            ResourceAuthority::Ena,
            "ena:ERR1",
            "release-1",
            vec![ResourceFile {
                file_id: "run-1".into(),
                filename: "fixture.fastq.gz".into(),
                bytes: 3,
                checksum: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into(),
                transport: Transport::Https,
            }],
            Destination {
                endpoint_id: "remote-appliance-1".into(),
                object_store_id: "remote-archive".into(),
                prefix: "bio/".into(),
                object_type: "dataset_file".into(),
            },
            "redistributable fixture",
            PolicyResult::Allowed,
        )
        .expect("plan");
        plan.confirm().expect("confirm");
        plan
    }
    fn snapshot() -> DestinationSnapshot {
        DestinationSnapshot {
            endpoint_id: "remote-appliance-1".into(),
            object_store_id: "remote-archive".into(),
            endpoint_label: "Studio appliance".into(),
            object_store_label: "Archive".into(),
        }
    }
    fn state() -> DestinationAuthorityState {
        DestinationAuthorityState {
            endpoint_id: "remote-appliance-1".into(),
            object_store_id: "remote-archive".into(),
            endpoint_label: "Studio appliance".into(),
            object_store_label: "Archive".into(),
            endpoint_present: true,
            object_store_present: true,
            tls_trusted: true,
            paired: true,
            pairing_expires_at_unix_seconds: 1000,
            ready: true,
            writable: true,
            quota_available_bytes: 1,
        }
    }
    #[test]
    fn confirmed_plan_streams_once_and_reuses_accession_file_checksum_commit() {
        let mut committer = ConfirmedBioinformaticsCommitter::new(Backend::default());
        let first = committer
            .commit_file(
                &plan(),
                &snapshot(),
                &state(),
                500,
                "run-1",
                "bio-ingest-1".into(),
                2,
                [b"ab".as_slice(), b"c".as_slice()],
            )
            .expect("commit");
        assert_eq!(first.object_key, "bio/fixture.fastq.gz");
        let second = committer
            .commit_file(
                &plan(),
                &snapshot(),
                &state(),
                500,
                "run-1",
                "bio-ingest-2".into(),
                2,
                std::iter::empty(),
            )
            .expect("deduplicated");
        assert_eq!(first, second);
        assert_eq!(committer.into_backend().chunks, 2);
    }
    #[test]
    fn blocks_unconfirmed_bad_destination_and_revalidation_before_streaming() {
        let mut committer = ConfirmedBioinformaticsCommitter::new(Backend::default());
        let mut unconfirmed = plan();
        unconfirmed.confirmed = false;
        assert_eq!(
            committer.commit_file(
                &unconfirmed,
                &snapshot(),
                &state(),
                500,
                "run-1",
                "bio-ingest-1".into(),
                2,
                std::iter::empty()
            ),
            Err(BioinformaticsCommitError::Unconfirmed)
        );
        let mut bad_state = state();
        bad_state.writable = false;
        assert!(matches!(
            committer.commit_file(
                &plan(),
                &snapshot(),
                &bad_state,
                500,
                "run-1",
                "bio-ingest-1".into(),
                2,
                std::iter::empty()
            ),
            Err(BioinformaticsCommitError::Revalidation(
                DestinationRevalidationError::ReadOnly
            ))
        ));
    }
}
