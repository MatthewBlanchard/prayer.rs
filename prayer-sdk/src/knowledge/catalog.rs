//! Canonical catalog parsing and bucket replacement.

use super::*;
#[cfg(test)]
pub use prayer_runtime::knowledge::CANONICAL_CATALOG_ROOTS;
pub use prayer_runtime::knowledge::{
    has_canonical_catalog, merge_unique_strings, validate_catalog_bucket_lengths,
};

pub fn canonical_catalog_from_cache(
    cache: &spacemolt_lib_rs::data::CatalogCache,
) -> Result<CatalogData, SdkError> {
    validate_catalog_bucket_lengths([
        ("items", cache.items().len()),
        ("ships", cache.ships().len()),
        ("recipes", cache.recipes().len()),
        ("facilities", cache.facilities().len()),
        ("skills", cache.skills().len()),
    ])
    .map_err(|error| SdkError::BadRequest(error.to_string()))?;
    Ok(CatalogData {
        version: Some(cache.version().to_string()),
        items: cache
            .items()
            .iter()
            .cloned()
            .map(|entry| (entry.id().to_string(), entry))
            .collect(),
        ships: cache
            .ships()
            .iter()
            .cloned()
            .map(|entry| (entry.id.clone(), entry))
            .collect(),
        recipes: cache
            .recipes()
            .iter()
            .cloned()
            .map(|entry| (entry.id.clone(), entry))
            .collect(),
        facilities: cache
            .facilities()
            .iter()
            .cloned()
            .map(|entry| (entry.id.clone(), entry))
            .collect(),
        skills: cache
            .skills()
            .iter()
            .cloned()
            .map(|entry| (entry.id.clone(), entry))
            .collect(),
    })
}

pub fn merge_map_vec_unique(
    dst: &mut HashMap<String, Vec<String>>,
    src: &HashMap<String, Vec<String>>,
) {
    for (key, values) in src {
        let entry = dst.entry(key.clone()).or_default();
        for value in values {
            if !entry.iter().any(|v| v == value) {
                entry.push(value.clone());
            }
        }
    }
}

pub fn merge_map_vec_unique_except_values(
    dst: &mut HashMap<String, Vec<String>>,
    src: &HashMap<String, Vec<String>>,
    excluded_values: &[&str],
) {
    for (key, values) in src {
        for value in values {
            if excluded_values.iter().any(|excluded| value == excluded) {
                continue;
            }
            let entry = dst.entry(key.clone()).or_default();
            if !entry.iter().any(|v| v == value) {
                entry.push(value.clone());
            }
        }
    }
}
