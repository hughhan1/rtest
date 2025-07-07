//! Collection reporting functionality.

use super::error::{CollectionError, CollectionOutcome};
use super::session::Session;
use super::types::Collector;

/// Collection report
#[derive(Debug)]
pub struct CollectReport {
    pub nodeid: String,
    pub outcome: CollectionOutcome,
    pub longrepr: Option<String>,
    pub error_type: Option<CollectionError>,
    pub result: Vec<Collector>,
}

impl CollectReport {
    pub fn new(
        nodeid: String,
        outcome: CollectionOutcome,
        longrepr: Option<String>,
        error_type: Option<CollectionError>,
        result: Vec<Collector>,
    ) -> Self {
        Self {
            nodeid,
            outcome,
            longrepr,
            error_type,
            result,
        }
    }
}

/// Collect a single node and return a report
pub fn collect_one_node(node: &Collector, session: &Session) -> CollectReport {
    match session.collect_node(node) {
        Ok(result) => CollectReport::new(
            node.nodeid().into(),
            CollectionOutcome::Passed,
            None,
            None,
            result,
        ),
        Err(e) => CollectReport::new(
            node.nodeid().into(),
            CollectionOutcome::Failed,
            Some(e.to_string()),
            Some(e),
            vec![],
        ),
    }
}