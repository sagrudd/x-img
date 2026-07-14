// SPDX-License-Identifier: MPL-2.0
//! Bounded, deterministic scheduling contracts without connector execution.

#![allow(missing_docs)] // Compact public record fields mirror the scheduler contract.

use std::collections::BTreeMap;

/// Family of work admitted by the common scheduler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum JobKind {
    /// A configured social account refresh.
    AccountRefresh,
    /// An explicitly observed browser capture.
    ExtensionCapture,
    /// An explicitly selected bioinformatics resource plan.
    ResourceCommit,
    /// An explicitly selected video candidate.
    VideoNormalization,
}

/// Stable mutually-exclusive source scope.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourceScope {
    /// The source-family scheduler lane.
    pub kind: JobKind,
    /// Stable account, origin, resource, or candidate identity.
    pub source_id: String,
}

impl SourceScope {
    /// Creates a bounded source scope.
    pub fn new(kind: JobKind, source_id: impl Into<String>) -> Result<Self, SchedulerError> {
        let scope = Self {
            kind,
            source_id: source_id.into(),
        };
        if !is_identifier(&scope.source_id) {
            return Err(SchedulerError::InvalidMetadata { field: "source_id" });
        }
        Ok(scope)
    }
}

/// Per-global job capacity and cost limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobBudget {
    /// Maximum active child leases.
    pub max_concurrent_children: u32,
    /// Maximum source requests reserved by children.
    pub max_requests: u64,
    /// Maximum bytes reserved by children.
    pub max_bytes: u64,
    /// Maximum elapsed logical seconds.
    pub max_duration_seconds: u64,
}

impl JobBudget {
    /// Rejects unbounded/zero capacity before a job is admitted.
    pub fn validate(self) -> Result<(), SchedulerError> {
        if self.max_concurrent_children == 0 || self.max_duration_seconds == 0 {
            return Err(SchedulerError::InvalidMetadata {
                field: "job_budget",
            });
        }
        Ok(())
    }
}

/// Explicit child state visible to a future host/UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildState {
    Pending,
    Claimed,
    Cancelled,
    Completed,
    Failed,
    PolicyBlocked,
}

/// Explicit global refresh state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalState {
    Active,
    Cancelled,
    Complete,
    Partial,
    Failed,
    PolicyBlocked,
}

/// A child-job lease; owner is an opaque worker reference, never a credential.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobLease {
    pub owner_id: String,
    pub expires_at: u64,
}

/// Observable child job contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildJob {
    pub scope: SourceScope,
    pub state: ChildState,
    pub lease: Option<JobLease>,
    pub requests_used: u64,
    pub bytes_used: u64,
}

/// Observable global refresh contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalRefresh {
    pub job_id: String,
    pub actor_scope: String,
    pub state: GlobalState,
    pub budget: JobBudget,
    pub children: BTreeMap<SourceScope, ChildJob>,
}

/// Result of a repeated global-refresh press.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshOutcome {
    Started { job_id: String },
    Coalesced { job_id: String },
}

/// Scheduler with one coalesced global refresh per actor scope.
#[derive(Debug, Default)]
pub struct Scheduler {
    active: BTreeMap<String, GlobalRefresh>,
    next_job: u64,
}

impl Scheduler {
    /// Starts a refresh or returns the existing active refresh for the actor.
    pub fn request_refresh(
        &mut self,
        actor_scope: impl Into<String>,
        sources: impl IntoIterator<Item = SourceScope>,
        budget: JobBudget,
    ) -> Result<RefreshOutcome, SchedulerError> {
        let actor_scope = actor_scope.into();
        if !is_identifier(&actor_scope) {
            return Err(SchedulerError::InvalidMetadata {
                field: "actor_scope",
            });
        }
        budget.validate()?;
        if let Some(job) = self.active.get(&actor_scope) {
            return Ok(RefreshOutcome::Coalesced {
                job_id: job.job_id.clone(),
            });
        }
        let mut children = BTreeMap::new();
        for scope in sources {
            if children
                .insert(
                    scope.clone(),
                    ChildJob {
                        scope,
                        state: ChildState::Pending,
                        lease: None,
                        requests_used: 0,
                        bytes_used: 0,
                    },
                )
                .is_some()
            {
                return Err(SchedulerError::DuplicateSource);
            }
        }
        let job_id = format!("refresh-{}", self.next_job);
        self.next_job += 1;
        self.active.insert(
            actor_scope.clone(),
            GlobalRefresh {
                job_id: job_id.clone(),
                actor_scope,
                state: GlobalState::Active,
                budget,
                children,
            },
        );
        Ok(RefreshOutcome::Started { job_id })
    }

    /// Claims one pending child if capacity and per-source exclusivity permit it.
    pub fn claim(
        &mut self,
        actor_scope: &str,
        scope: &SourceScope,
        owner_id: impl Into<String>,
        now: u64,
        ttl_seconds: u64,
    ) -> Result<(), SchedulerError> {
        let owner_id = owner_id.into();
        if !is_identifier(&owner_id) || ttl_seconds == 0 {
            return Err(SchedulerError::InvalidMetadata { field: "lease" });
        }
        let job = self
            .active
            .get_mut(actor_scope)
            .ok_or(SchedulerError::NoActiveRefresh)?;
        if job.state != GlobalState::Active {
            return Err(SchedulerError::GlobalNotActive);
        }
        let active_count = job
            .children
            .values()
            .filter(|child| child.state == ChildState::Claimed)
            .count();
        let child = job
            .children
            .get_mut(scope)
            .ok_or(SchedulerError::UnknownSource)?;
        if child.state == ChildState::Claimed
            && child
                .lease
                .as_ref()
                .is_some_and(|lease| lease.expires_at > now)
        {
            return Err(SchedulerError::SourceAlreadyClaimed);
        }
        if child.state != ChildState::Pending && child.state != ChildState::Claimed {
            return Err(SchedulerError::ChildNotClaimable);
        }
        if child.state == ChildState::Pending
            && active_count
                >= usize::try_from(job.budget.max_concurrent_children).expect("u32 fits usize")
        {
            return Err(SchedulerError::CapacityLimited);
        }
        child.state = ChildState::Claimed;
        child.lease = Some(JobLease {
            owner_id,
            expires_at: now.saturating_add(ttl_seconds),
        });
        Ok(())
    }

    /// Records bounded cost evidence for the current lease owner.
    pub fn record_usage(
        &mut self,
        actor_scope: &str,
        scope: &SourceScope,
        owner_id: &str,
        requests: u64,
        bytes: u64,
        elapsed_seconds: u64,
    ) -> Result<(), SchedulerError> {
        let job = self
            .active
            .get_mut(actor_scope)
            .ok_or(SchedulerError::NoActiveRefresh)?;
        let total_requests: u64 = job.children.values().map(|child| child.requests_used).sum();
        let total_bytes: u64 = job.children.values().map(|child| child.bytes_used).sum();
        if total_requests.saturating_add(requests) > job.budget.max_requests
            || total_bytes.saturating_add(bytes) > job.budget.max_bytes
            || elapsed_seconds > job.budget.max_duration_seconds
        {
            return Err(SchedulerError::BudgetExceeded);
        }
        let child = job
            .children
            .get_mut(scope)
            .ok_or(SchedulerError::UnknownSource)?;
        if child.state != ChildState::Claimed
            || child
                .lease
                .as_ref()
                .is_none_or(|lease| lease.owner_id != owner_id)
        {
            return Err(SchedulerError::LeaseNotOwned);
        }
        child.requests_used = child.requests_used.saturating_add(requests);
        child.bytes_used = child.bytes_used.saturating_add(bytes);
        Ok(())
    }

    /// Cooperatively cancels every non-terminal child and releases its lease.
    pub fn cancel(&mut self, actor_scope: &str) -> Result<(), SchedulerError> {
        let job = self
            .active
            .get_mut(actor_scope)
            .ok_or(SchedulerError::NoActiveRefresh)?;
        if job.state != GlobalState::Active {
            return Err(SchedulerError::GlobalNotActive);
        }
        for child in job.children.values_mut() {
            if matches!(child.state, ChildState::Pending | ChildState::Claimed) {
                child.state = ChildState::Cancelled;
                child.lease = None;
            }
        }
        job.state = GlobalState::Cancelled;
        Ok(())
    }

    /// Returns the active job for inspection without exposing connector secrets.
    #[must_use]
    pub fn active(&self, actor_scope: &str) -> Option<&GlobalRefresh> {
        self.active.get(actor_scope)
    }
}

/// Scheduler contract rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerError {
    InvalidMetadata { field: &'static str },
    DuplicateSource,
    NoActiveRefresh,
    GlobalNotActive,
    UnknownSource,
    SourceAlreadyClaimed,
    ChildNotClaimable,
    CapacityLimited,
    LeaseNotOwned,
    BudgetExceeded,
}

impl std::fmt::Display for SchedulerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "scheduler contract rejected: {self:?}")
    }
}
impl std::error::Error for SchedulerError {}

fn is_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value.chars().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | ':' | '-')
        })
}

#[cfg(test)]
mod tests {
    use super::{
        ChildState, JobBudget, JobKind, RefreshOutcome, Scheduler, SchedulerError, SourceScope,
    };
    fn scope(id: &str) -> SourceScope {
        SourceScope::new(JobKind::AccountRefresh, id).expect("scope")
    }
    fn budget() -> JobBudget {
        JobBudget {
            max_concurrent_children: 1,
            max_requests: 10,
            max_bytes: 100,
            max_duration_seconds: 60,
        }
    }
    #[test]
    fn concurrent_refresh_presses_coalesce_and_sources_never_overlap() {
        let mut scheduler = Scheduler::default();
        let first = scheduler
            .request_refresh("actor", [scope("account-a"), scope("account-b")], budget())
            .expect("start");
        let repeated = scheduler
            .request_refresh("actor", [scope("account-a")], budget())
            .expect("coalesce");
        assert!(
            matches!((first, repeated), (RefreshOutcome::Started { job_id: a }, RefreshOutcome::Coalesced { job_id: b }) if a == b)
        );
        scheduler
            .claim("actor", &scope("account-a"), "worker-a", 10, 20)
            .expect("first claim");
        assert_eq!(
            scheduler.claim("actor", &scope("account-a"), "worker-b", 11, 20),
            Err(SchedulerError::SourceAlreadyClaimed)
        );
        assert_eq!(
            scheduler.claim("actor", &scope("account-b"), "worker-b", 11, 20),
            Err(SchedulerError::CapacityLimited)
        );
    }
    #[test]
    fn budgets_and_cancellation_are_bounded_and_release_leases() {
        let mut scheduler = Scheduler::default();
        scheduler
            .request_refresh("actor", [scope("account-a")], budget())
            .expect("start");
        scheduler
            .claim("actor", &scope("account-a"), "worker-a", 10, 20)
            .expect("claim");
        scheduler
            .record_usage("actor", &scope("account-a"), "worker-a", 5, 50, 20)
            .expect("within budget");
        assert_eq!(
            scheduler.record_usage("actor", &scope("account-a"), "worker-a", 6, 0, 20),
            Err(SchedulerError::BudgetExceeded)
        );
        scheduler.cancel("actor").expect("cancel");
        let child = scheduler
            .active("actor")
            .expect("job")
            .children
            .get(&scope("account-a"))
            .expect("child");
        assert_eq!(child.state, ChildState::Cancelled);
        assert!(child.lease.is_none());
    }
}
