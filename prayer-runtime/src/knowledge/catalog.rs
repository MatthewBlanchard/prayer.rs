use std::collections::HashMap;

use thiserror::Error;

use crate::CatalogData;

pub const CANONICAL_CATALOG_ROOTS: &[&str] = &["items", "ships", "recipes", "facilities", "skills"];

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CatalogError {
    #[error("canonical catalog missing required buckets: {0}")]
    MissingBuckets(String),
}

pub fn validate_catalog_bucket_lengths(
    lengths: impl IntoIterator<Item = (&'static str, usize)>,
) -> Result<(), CatalogError> {
    let missing = lengths
        .into_iter()
        .filter(|(_, len)| *len == 0)
        .map(|(bucket, len)| format!("{bucket}={len}"))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(CatalogError::MissingBuckets(missing.join(", ")))
    }
}

pub fn merge_unique_strings(dst: &mut Vec<String>, src: &[String]) {
    for value in src {
        if !dst.iter().any(|existing| existing == value) {
            dst.push(value.clone());
        }
    }
}

pub fn replace_catalog_bucket<T: Clone>(
    ids: &mut Vec<String>,
    entries: &mut HashMap<String, T>,
    fetched: &HashMap<String, T>,
) {
    if fetched.is_empty() {
        return;
    }
    let mut fetched_ids = fetched.keys().cloned().collect::<Vec<_>>();
    fetched_ids.sort();
    *ids = fetched_ids;
    *entries = fetched.clone();
}

pub fn has_canonical_catalog(catalog: &CatalogData) -> bool {
    !catalog.items.is_empty()
        && !catalog.ships.is_empty()
        && !catalog.recipes.is_empty()
        && !catalog.facilities.is_empty()
        && !catalog.skills.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_every_canonical_bucket() {
        let error = validate_catalog_bucket_lengths([("items", 0), ("ships", 1)])
            .expect_err("incomplete catalog");
        assert!(matches!(error, CatalogError::MissingBuckets(_)));
    }
}
