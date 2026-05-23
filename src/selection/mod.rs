//! Post-collection test selection (`-k`, `--lf`, `--ff`).

mod expression;
mod keyword;

use crate::cache::{apply_last_failed, CacheError, LastFailedMode, LastFailedNoFailures};
use crate::collection_integration::CollectedItem;

pub use expression::ParseError;

/// Apply all post-collection selection steps in order: cache modes, then `-k`.
pub fn apply_selection(
    items: Vec<CollectedItem>,
    keyword_expr: Option<&str>,
    last_failed: bool,
    failed_first: bool,
    lfnf: LastFailedNoFailures,
    cache_dir: &std::path::Path,
) -> Result<(Vec<CollectedItem>, SelectionStats), SelectionError> {
    let initial_count = items.len();
    let mut current = items;

    if last_failed || failed_first {
        let mode = if last_failed {
            LastFailedMode::LastFailed
        } else {
            LastFailedMode::FailedFirst
        };
        current =
            apply_last_failed(current, mode, lfnf, cache_dir).map_err(SelectionError::Cache)?;
    }

    let after_cache_count = current.len();
    let mut deselected_by_keyword = 0usize;

    if let Some(expr) = keyword_expr {
        if expr.is_empty() {
            return Err(SelectionError::KeywordParse(ParseError {
                column: 1,
                message: "empty expression".into(),
            }));
        }
        let compiled =
            expression::compile_expression(expr).map_err(SelectionError::KeywordParse)?;
        let before = current.len();
        current = apply_keyword_filter(current, &compiled)?;
        deselected_by_keyword = before.saturating_sub(current.len());
    }

    let deselected = initial_count
        .saturating_sub(after_cache_count)
        .saturating_add(deselected_by_keyword);

    Ok((
        current,
        SelectionStats {
            deselected,
            deselected_by_keyword,
        },
    ))
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SelectionStats {
    pub deselected: usize,
    pub deselected_by_keyword: usize,
}

#[derive(Debug)]
pub enum SelectionError {
    KeywordParse(ParseError),
    Cache(CacheError),
}

impl std::fmt::Display for SelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SelectionError::KeywordParse(e) => write!(f, "Invalid -k expression {e}"),
            SelectionError::Cache(e) => write!(f, "Cache error: {e}"),
        }
    }
}

pub fn apply_keyword_filter(
    items: Vec<CollectedItem>,
    expr: &expression::CompiledExpression,
) -> Result<Vec<CollectedItem>, SelectionError> {
    let mut kept = Vec::new();
    for item in items {
        let matches =
            expression::evaluate_compiled(expr, |ident| keyword::item_matches_ident(&item, ident));
        if matches {
            kept.push(item);
        }
    }
    Ok(kept)
}
