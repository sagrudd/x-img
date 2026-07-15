// SPDX-License-Identifier: MPL-2.0
//! Bounded, redacted operational health, metrics, and audit model.
//!
//! Every field is an enum or bounded counter. There is deliberately no
//! free-form message, URL, actor, session, object key, checksum, or payload.

use std::collections::{BTreeMap, VecDeque};

use serde::Serialize;

/// Maximum recent audit facts retained in memory.
pub const AUDIT_CAPACITY: usize = 128;

/// Stable operational component names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Component {
    /// Host context supplied by Monas or a future approved host.
    HostContext,
    /// DASObjectStore authority access.
    ObjectStore,
    /// Bounded scheduler and workers.
    Scheduler,
    /// Containerized video normalizer.
    Normalizer,
    /// Firefox capture/cache adapter boundary.
    Firefox,
}

/// Word-first health state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthState {
    /// Component is ready for its configured work.
    Ready,
    /// Component works partially or needs operator attention.
    Degraded,
    /// Component is unavailable.
    Unavailable,
}

/// Fixed event codes safe for diagnostics and audit export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventCode {
    /// Host-authorized request admitted.
    RequestAdmitted,
    /// Request rejected at an authorization/policy boundary.
    RequestRejected,
    /// Object authority operation verified.
    ObjectVerified,
    /// Object authority unavailable or returned no object.
    ObjectUnavailable,
    /// Job entered a terminal successful state.
    JobCompleted,
    /// Job failed or was cancelled.
    JobFailed,
    /// Compliance tombstone/removal transition verified.
    ComplianceTransition,
    /// Browser substitution fell back to the origin.
    OriginFallback,
}

/// Fixed outcome, separate from colour or prose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventOutcome {
    /// Operation succeeded.
    Success,
    /// Operation was deliberately blocked.
    Blocked,
    /// Operation is waiting/retryable.
    Pending,
    /// Operation failed.
    Failed,
}

/// One redacted recent audit fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AuditFact {
    /// Monotonic in-process sequence; not a user or request identity.
    pub sequence: u64,
    /// Typed component.
    pub component: Component,
    /// Typed event code.
    pub code: EventCode,
    /// Typed outcome.
    pub outcome: EventOutcome,
}

/// One aggregate counter row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct MetricCount {
    /// Typed component.
    pub component: Component,
    /// Typed event code.
    pub code: EventCode,
    /// Saturating count since process start.
    pub count: u64,
}

/// One component-health row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ComponentHealth {
    /// Component name.
    pub component: Component,
    /// Current word-first state.
    pub state: HealthState,
}

/// Authenticated operational snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OperationsSnapshot {
    /// Worst current state across components.
    pub status: HealthState,
    /// Deterministically ordered component states.
    pub components: Vec<ComponentHealth>,
    /// Deterministically ordered aggregate counts.
    pub metrics: Vec<MetricCount>,
    /// Bounded newest audit facts.
    pub audit: Vec<AuditFact>,
    /// Number of older facts discarded due to the fixed capacity.
    pub dropped_audit_facts: u64,
}

/// In-memory operations recorder. Durable audit persistence belongs to the
/// authenticated host and must retain the same redacted schema.
#[derive(Debug, Clone)]
pub struct OperationalTelemetry {
    health: BTreeMap<Component, HealthState>,
    metrics: BTreeMap<(Component, EventCode), u64>,
    audit: VecDeque<AuditFact>,
    next_sequence: u64,
    dropped_audit_facts: u64,
}

impl Default for OperationalTelemetry {
    fn default() -> Self {
        let health = [
            Component::HostContext,
            Component::ObjectStore,
            Component::Scheduler,
            Component::Normalizer,
            Component::Firefox,
        ]
        .into_iter()
        .map(|component| (component, HealthState::Unavailable))
        .collect();
        Self {
            health,
            metrics: BTreeMap::new(),
            audit: VecDeque::with_capacity(AUDIT_CAPACITY),
            next_sequence: 1,
            dropped_audit_facts: 0,
        }
    }
}

impl OperationalTelemetry {
    /// Updates one typed component state.
    pub fn set_health(&mut self, component: Component, state: HealthState) {
        self.health.insert(component, state);
    }

    /// Records one typed event, with saturating metrics and bounded audit.
    pub fn record(&mut self, component: Component, code: EventCode, outcome: EventOutcome) {
        let counter = self.metrics.entry((component, code)).or_default();
        *counter = counter.saturating_add(1);
        if self.audit.len() == AUDIT_CAPACITY {
            self.audit.pop_front();
            self.dropped_audit_facts = self.dropped_audit_facts.saturating_add(1);
        }
        self.audit.push_back(AuditFact {
            sequence: self.next_sequence,
            component,
            code,
            outcome,
        });
        self.next_sequence = self.next_sequence.saturating_add(1);
    }

    /// Returns a deterministic redacted snapshot.
    #[must_use]
    pub fn snapshot(&self) -> OperationsSnapshot {
        OperationsSnapshot {
            status: overall(self.health.values().copied()),
            components: self
                .health
                .iter()
                .map(|(&component, &state)| ComponentHealth { component, state })
                .collect(),
            metrics: self
                .metrics
                .iter()
                .map(|(&(component, code), &count)| MetricCount {
                    component,
                    code,
                    count,
                })
                .collect(),
            audit: self.audit.iter().copied().collect(),
            dropped_audit_facts: self.dropped_audit_facts,
        }
    }
}

fn overall(states: impl Iterator<Item = HealthState>) -> HealthState {
    states.fold(HealthState::Ready, |current, state| {
        match (current, state) {
            (HealthState::Unavailable, _) | (_, HealthState::Unavailable) => {
                HealthState::Unavailable
            }
            (HealthState::Degraded, _) | (_, HealthState::Degraded) => HealthState::Degraded,
            _ => HealthState::Ready,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_is_worded_and_worst_component_wins() {
        let mut telemetry = OperationalTelemetry::default();
        for component in [
            Component::HostContext,
            Component::ObjectStore,
            Component::Scheduler,
            Component::Normalizer,
            Component::Firefox,
        ] {
            telemetry.set_health(component, HealthState::Ready);
        }
        assert_eq!(telemetry.snapshot().status, HealthState::Ready);
        telemetry.set_health(Component::ObjectStore, HealthState::Degraded);
        assert_eq!(telemetry.snapshot().status, HealthState::Degraded);
        telemetry.set_health(Component::Scheduler, HealthState::Unavailable);
        assert_eq!(telemetry.snapshot().status, HealthState::Unavailable);
    }

    #[test]
    fn metrics_saturate_and_audit_is_bounded_without_free_form_data() {
        let mut telemetry = OperationalTelemetry::default();
        for _ in 0..(AUDIT_CAPACITY + 5) {
            telemetry.record(
                Component::Firefox,
                EventCode::OriginFallback,
                EventOutcome::Pending,
            );
        }
        let snapshot = telemetry.snapshot();
        assert_eq!(snapshot.audit.len(), AUDIT_CAPACITY);
        assert_eq!(snapshot.dropped_audit_facts, 5);
        assert_eq!(snapshot.metrics[0].count, (AUDIT_CAPACITY + 5) as u64);
        let json = serde_json::to_string(&snapshot).unwrap();
        for prohibited in [
            "http",
            "cookie",
            "authorization",
            "checksum",
            "actor",
            "session",
        ] {
            assert!(!json.contains(prohibited));
        }
    }
}
