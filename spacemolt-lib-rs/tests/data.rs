use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{json, Value};
use spacemolt_lib_rs::auth::MemoryCredentialStore;
use spacemolt_lib_rs::client::{SpacemoltClient, SpacemoltClientOptions};
use spacemolt_lib_rs::data::{http_base_from_ws, CatalogCache, DataHttpClient, MapCache};

#[derive(Default)]
struct MockDataHttp {
    calls: Mutex<Vec<String>>,
}

#[async_trait]
impl DataHttpClient for MockDataHttp {
    async fn get_json(&self, url: &str) -> Result<Value, String> {
        self.calls.lock().expect("calls").push(url.to_string());
        if url.ends_with("/api/catalog.json") {
            return Ok(json!({
                "version": "0.452.0",
                "achievements": [],
                "faction_achievements": [],
                "hidden_achievement_count": 0,
                "hidden_faction_achievement_count": 0,
                "ships": [],
                "items": [],
                "skills": [],
                "recipes": [],
                "facilities": []
            }));
        }
        if url.ends_with("/api/map") {
            return Ok(json!({
                "systems": [],
                "empires": { "solarian": "#ffd700" }
            }));
        }
        Err(format!("unmatched GET {url}"))
    }
}

impl MockDataHttp {
    fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("calls").clone()
    }
}

#[test]
fn http_base_from_ws_derives_the_http_origin() {
    assert_eq!(
        http_base_from_ws("wss://game.spacemolt.com/ws/v2"),
        "https://game.spacemolt.com"
    );
    assert_eq!(
        http_base_from_ws("ws://localhost:8080/ws/v2"),
        "http://localhost:8080"
    );
}

#[test]
fn client_options_derive_both_endpoints_from_one_origin() {
    let secure = SpacemoltClientOptions::from_origin("https://example.test/");
    assert_eq!(secure.http_base_url, "https://example.test");
    assert_eq!(secure.url, "wss://example.test/ws/v2");

    let local = SpacemoltClientOptions::from_origin("http://localhost:8080");
    assert_eq!(local.http_base_url, "http://localhost:8080");
    assert_eq!(local.url, "ws://localhost:8080/ws/v2");
}

#[test]
fn catalog_cache_normalizes_missing_sections() {
    let cache = CatalogCache::new(json!({
        "version": "0.452.0",
        "achievements": [],
        "faction_achievements": [],
        "hidden_achievement_count": 0,
        "hidden_faction_achievement_count": 0,
        "ships": [],
        "items": [],
        "skills": [],
        "recipes": [],
        "facilities": []
    }))
    .expect("valid catalog");

    assert_eq!(cache.version(), "0.452.0");
    assert!(cache.ship("nope").is_none());
    assert!(cache.item("nope").is_none());
    assert!(cache.ships().is_empty());
    assert!(cache.recipes().is_empty());
}

#[test]
fn catalog_cache_adapts_public_ship_build_material_rows() {
    let cache = CatalogCache::new(json!({
        "version": "0.524.0",
        "achievements": [],
        "faction_achievements": [],
        "hidden_achievement_count": 0,
        "hidden_faction_achievement_count": 0,
        "ships": [{
            "id": "scout",
            "name": "Scout",
            "class": "Scout",
            "build_materials": [
                { "item_id": "steel_plate", "quantity": 5 },
                { "item_id": "copper_wiring", "quantity": 2 }
            ]
        }],
        "items": [],
        "skills": [],
        "recipes": [],
        "facilities": []
    }))
    .expect("live public catalog shape");

    let materials = &cache.ship("scout").expect("scout").build_materials;
    assert_eq!(materials["steel_plate"], 5);
    assert_eq!(materials["copper_wiring"], 2);
}

#[test]
fn catalog_cache_preserves_documented_ship_build_material_maps() {
    let cache = CatalogCache::new(json!({
        "version": "0.524.0",
        "achievements": [],
        "faction_achievements": [],
        "hidden_achievement_count": 0,
        "hidden_faction_achievement_count": 0,
        "ships": [{
            "id": "scout",
            "name": "Scout",
            "class": "Scout",
            "build_materials": { "steel_plate": 5 }
        }],
        "items": [],
        "skills": [],
        "recipes": [],
        "facilities": []
    }))
    .expect("documented CatalogDump shape");

    assert_eq!(
        cache.ship("scout").expect("scout").build_materials["steel_plate"],
        5
    );
}

#[test]
fn catalog_errors_include_the_failing_field_path() {
    let error = CatalogCache::new(json!({
        "version": "0.452.0",
        "achievements": [], "faction_achievements": [],
        "hidden_achievement_count": 0, "hidden_faction_achievement_count": 0,
        "ships": [], "items": [], "skills": [], "recipes": [],
        "facilities": "not-an-array"
    }))
    .expect_err("invalid facilities shape");

    assert!(error.contains("facilities"), "{error}");
}

#[test]
fn map_cache_preserves_empires_and_normalizes_empty_systems() {
    let cache = MapCache::new(json!({
        "systems": [],
        "empires": { "solarian": "#ffd700" }
    }))
    .expect("valid map");

    assert!(cache.system("sol").is_none());
    assert!(cache.systems().is_empty());
    assert_eq!(cache.empires()["solarian"], "#ffd700");
}

#[tokio::test]
async fn client_catalog_and_map_fetch_once_and_can_force_reload() {
    let http = Arc::new(MockDataHttp::default());
    let client = SpacemoltClient::new(
        SpacemoltClientOptions {
            url: "wss://game.spacemolt.com/ws/v2".to_string(),
            data_http_client: Some(http.clone()),
            ..SpacemoltClientOptions::default()
        },
        MemoryCredentialStore::default(),
    );

    let catalog_1 = client.catalog(false).await.expect("catalog");
    let catalog_2 = client.catalog(false).await.expect("catalog cached");
    assert!(Arc::ptr_eq(&catalog_1, &catalog_2));
    assert!(catalog_1.ships().is_empty());
    assert_eq!(
        http.calls()
            .iter()
            .filter(|url| url.ends_with("/api/catalog.json"))
            .count(),
        1
    );

    let map = client.map(false).await.expect("map");
    assert!(map.systems().is_empty());
    let catalog_3 = client.catalog(true).await.expect("catalog reload");
    assert!(!Arc::ptr_eq(&catalog_1, &catalog_3));
    assert_eq!(
        http.calls()
            .iter()
            .filter(|url| url.ends_with("/api/catalog.json"))
            .count(),
        2
    );
}

#[tokio::test]
async fn concurrent_initial_catalog_reads_are_single_flight() {
    let http = Arc::new(MockDataHttp::default());
    let client = SpacemoltClient::new(
        SpacemoltClientOptions {
            data_http_client: Some(http.clone()),
            ..SpacemoltClientOptions::default()
        },
        MemoryCredentialStore::default(),
    );

    let (first, second, third) = tokio::join!(
        client.catalog(false),
        client.catalog(false),
        client.catalog(false)
    );
    let first = first.expect("first catalog");
    let second = second.expect("second catalog");
    let third = third.expect("third catalog");

    assert!(Arc::ptr_eq(&first, &second));
    assert!(Arc::ptr_eq(&first, &third));
    assert_eq!(
        http.calls()
            .iter()
            .filter(|url| url.ends_with("/api/catalog.json"))
            .count(),
        1
    );
}
