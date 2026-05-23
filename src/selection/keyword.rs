//! Keyword matching for `-k` expressions.

use crate::collection_integration::CollectedItem;

/// Returns true if `ident` matches any keyword (case-insensitive substring).
///
/// Keywords are stored lowercased at collection time; only `ident` is normalized per lookup.
pub fn ident_matches_keywords(ident: &str, keywords: &[String]) -> bool {
    let ident_lower = ident.to_lowercase();
    keywords.iter().any(|kw| kw.contains(&ident_lower))
}

/// Returns true if the item matches the given identifier.
pub fn item_matches_ident(item: &CollectedItem, ident: &str) -> bool {
    ident_matches_keywords(ident, &item.keywords)
}
