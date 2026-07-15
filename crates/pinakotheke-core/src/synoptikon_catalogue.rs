// SPDX-License-Identifier: MPL-2.0
//! Bounded, project-scoped catalogue projection for a Synoptikon host.

#![allow(missing_docs)]

use serde::{Deserialize, Serialize};

use crate::host_context::{AuthenticatedHostContext, HostMode};

pub const SYNOPTIKON_CATALOGUE_SCHEMA: &str = "pinakotheke.synoptikon-catalogue.v1";
pub const SYNOPTIKON_CATALOGUE_READ: &str = "ximg.catalogue.read";
pub const MAX_CATALOGUE_PAGE_SIZE: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SynoptikonCatalogueItem {
    pub catalogue_id: String,
    pub project_id: String,
    pub media_kind: CatalogueMediaKind,
    pub review_state: CatalogueReviewState,
    pub source_label: String,
    pub discovered_at_epoch_seconds: u64,
    pub endpoint_id: String,
    pub object_store_id: String,
    pub object_key: String,
    pub checksum: String,
    pub content_type: String,
    pub content_length: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogueMediaKind {
    Image,
    Video,
    BioinformaticsResource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogueReviewState {
    NeedsReview,
    Accepted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SynoptikonCataloguePage {
    pub schema_version: &'static str,
    pub project_id: String,
    pub items: Vec<SynoptikonCatalogueItem>,
    pub next_offset: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SynoptikonCatalogueError {
    Unauthorized,
    InvalidScope,
    InvalidPageSize,
}

#[derive(Debug, Clone, Default)]
pub struct SynoptikonCatalogueProjection {
    items: Vec<SynoptikonCatalogueItem>,
}

impl SynoptikonCatalogueProjection {
    pub fn new(mut items: Vec<SynoptikonCatalogueItem>) -> Self {
        items.sort_by(|left, right| left.catalogue_id.cmp(&right.catalogue_id));
        Self { items }
    }

    pub fn page(
        &self,
        context: &AuthenticatedHostContext,
        offset: usize,
        limit: usize,
    ) -> Result<SynoptikonCataloguePage, SynoptikonCatalogueError> {
        if context.host_mode() != HostMode::SynoptikonIntegrated
            || !context.permits(SYNOPTIKON_CATALOGUE_READ)
        {
            return Err(SynoptikonCatalogueError::Unauthorized);
        }
        if limit == 0 || limit > MAX_CATALOGUE_PAGE_SIZE {
            return Err(SynoptikonCatalogueError::InvalidPageSize);
        }
        let scope = context
            .synoptikon_scope()
            .ok_or(SynoptikonCatalogueError::InvalidScope)?;
        let project = self
            .items
            .iter()
            .filter(|item| item.project_id == scope.project_id());
        let total = project.clone().count();
        let items = project
            .skip(offset)
            .take(limit)
            .cloned()
            .collect::<Vec<_>>();
        let next_offset = (offset + items.len() < total).then_some(offset + items.len());
        Ok(SynoptikonCataloguePage {
            schema_version: SYNOPTIKON_CATALOGUE_SCHEMA,
            project_id: scope.project_id().to_owned(),
            items,
            next_offset,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_context::{HostContextAdapter, SynoptikonHostContextAdapter};

    fn item(id: &str, project_id: &str) -> SynoptikonCatalogueItem {
        SynoptikonCatalogueItem {
            catalogue_id: id.into(),
            project_id: project_id.into(),
            media_kind: CatalogueMediaKind::Image,
            review_state: CatalogueReviewState::Accepted,
            source_label: "Synthetic fixture".into(),
            discovered_at_epoch_seconds: 1,
            endpoint_id: "endpoint-1".into(),
            object_store_id: "store-1".into(),
            object_key: format!("objects/{id}"),
            checksum: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .into(),
            content_type: "image/png".into(),
            content_length: 12,
        }
    }

    #[test]
    fn returns_only_the_authenticated_project_with_stable_pagination() {
        let context = SynoptikonHostContextAdapter
            .authenticate(include_bytes!(
                "../../../fixtures/host-context/v1/synoptikon-valid.json"
            ))
            .expect("valid host context");
        let projection = SynoptikonCatalogueProjection::new(vec![
            item("media-b", "synthetic-project"),
            item("hidden", "another-project"),
            item("media-a", "synthetic-project"),
        ]);
        let page = projection.page(&context, 0, 1).expect("authorized page");
        assert_eq!(page.items[0].catalogue_id, "media-a");
        assert_eq!(page.next_offset, Some(1));
        assert_eq!(
            projection.page(&context, 1, 1).unwrap().items[0].catalogue_id,
            "media-b"
        );
    }

    #[test]
    fn rejects_unbounded_pages() {
        let context = SynoptikonHostContextAdapter
            .authenticate(include_bytes!(
                "../../../fixtures/host-context/v1/synoptikon-valid.json"
            ))
            .unwrap();
        assert_eq!(
            SynoptikonCatalogueProjection::default().page(&context, 0, MAX_CATALOGUE_PAGE_SIZE + 1),
            Err(SynoptikonCatalogueError::InvalidPageSize)
        );
    }
}
