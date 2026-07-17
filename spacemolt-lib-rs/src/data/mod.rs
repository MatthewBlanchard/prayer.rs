//! Bulk HTTP data caches backed by generated OpenAPI DTOs.

use std::collections::HashMap;

use async_trait::async_trait;
use serde_json::Value;

use crate::schema::{
    CatalogDump, CatalogDumpItemsItem, FacilityDefinition, MapData, MapSystem, Recipe, ShipClass,
    SkillDefinition,
};
/// Canonical current market depth row. Buy/sell side is carried by its book.
pub type MarketOrder = crate::schema::OrderLevel;
pub type WreckCargoItem = crate::schema::ShipCargoItem;
pub type WreckModule = crate::schema::LootedModule;

/// Current location of SpaceMolt's mobile base.
pub use crate::schema::MobileBaseLocation;

/// Fetch the current mobile-base location through the client's readonly-data boundary.
pub async fn load_mobile_base_location(
    http_base_url: &str,
    http: &dyn DataHttpClient,
) -> Result<MobileBaseLocation, String> {
    let value = http
        .get_json(&format!(
            "{}/wheres-mobile-base",
            trim_trailing_slash(http_base_url)
        ))
        .await?;
    serde_json::from_value(value)
        .map_err(|err| format!("mobile base location returned invalid JSON: {err}"))
}

/// Raw JSON HTTP boundary. Domain caches deserialize its response immediately.
#[async_trait]
pub trait DataHttpClient: Send + Sync {
    async fn get_json(&self, url: &str) -> Result<Value, String>;
}

#[derive(Debug, Clone, Default)]
pub struct ReqwestDataHttpClient {
    client: reqwest::Client,
}

#[async_trait]
impl DataHttpClient for ReqwestDataHttpClient {
    async fn get_json(&self, url: &str) -> Result<Value, String> {
        let response = self
            .client
            .get(url)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(|err| format!("GET {url} failed: {err}"))?;
        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(format!("GET {url} -> {status}: {text}"));
        }
        response
            .json::<Value>()
            .await
            .map_err(|err| format!("GET {url} returned invalid JSON: {err}"))
    }
}

/// Local typed cache of `/api/catalog.json`.
#[derive(Debug, Clone)]
pub struct CatalogCache {
    version: String,
    ships: Vec<ShipClass>,
    items: Vec<CatalogDumpItemsItem>,
    recipes: Vec<Recipe>,
    skills: Vec<SkillDefinition>,
    facilities: Vec<FacilityDefinition>,
    ship_index: HashMap<String, usize>,
    item_index: HashMap<String, usize>,
    recipe_index: HashMap<String, usize>,
    skill_index: HashMap<String, usize>,
    facility_index: HashMap<String, usize>,
}

impl CatalogCache {
    pub fn new(catalog: Value) -> Result<Self, String> {
        let catalog = adapt_public_catalog_dump(catalog)?;
        let catalog: CatalogDump = serde_path_to_error::deserialize(catalog).map_err(|err| {
            format!(
                "catalog returned invalid data at {}: {}",
                err.path(),
                err.inner()
            )
        })?;
        Ok(Self {
            ship_index: index_ids(catalog.ships.iter().map(|entry| entry.id.as_str())),
            item_index: index_ids(catalog.items.iter().map(CatalogDumpItemsItem::id)),
            recipe_index: index_ids(catalog.recipes.iter().map(|entry| entry.id.as_str())),
            skill_index: index_ids(catalog.skills.iter().map(|entry| entry.id.as_str())),
            facility_index: index_ids(catalog.facilities.iter().map(|entry| entry.id.as_str())),
            version: catalog.version,
            ships: catalog.ships,
            items: catalog.items,
            recipes: catalog.recipes,
            skills: catalog.skills,
            facilities: catalog.facilities,
        })
    }

    pub async fn load(http_base_url: &str, http: &dyn DataHttpClient) -> Result<Self, String> {
        let data = http
            .get_json(&format!(
                "{}/api/catalog.json",
                trim_trailing_slash(http_base_url)
            ))
            .await?;
        Self::new(data)
    }

    pub fn version(&self) -> &str {
        &self.version
    }
    pub fn ship(&self, id: &str) -> Option<&ShipClass> {
        self.ship_index.get(id).map(|&i| &self.ships[i])
    }
    pub fn item(&self, id: &str) -> Option<&CatalogDumpItemsItem> {
        self.item_index.get(id).map(|&i| &self.items[i])
    }
    pub fn recipe(&self, id: &str) -> Option<&Recipe> {
        self.recipe_index.get(id).map(|&i| &self.recipes[i])
    }
    pub fn skill(&self, id: &str) -> Option<&SkillDefinition> {
        self.skill_index.get(id).map(|&i| &self.skills[i])
    }
    pub fn facility(&self, id: &str) -> Option<&FacilityDefinition> {
        self.facility_index.get(id).map(|&i| &self.facilities[i])
    }
    pub fn ships(&self) -> &[ShipClass] {
        &self.ships
    }
    pub fn items(&self) -> &[CatalogDumpItemsItem] {
        &self.items
    }
    pub fn recipes(&self) -> &[Recipe] {
        &self.recipes
    }
    pub fn skills(&self) -> &[SkillDefinition] {
        &self.skills
    }
    pub fn facilities(&self) -> &[FacilityDefinition] {
        &self.facilities
    }
}

fn adapt_public_catalog_dump(mut catalog: Value) -> Result<Value, String> {
    let Some(ships) = catalog.get_mut("ships").and_then(Value::as_array_mut) else {
        return Ok(catalog);
    };
    for (ship_index, ship) in ships.iter_mut().enumerate() {
        let Some(materials) = ship.get_mut("build_materials") else {
            continue;
        };
        let Value::Array(rows) = materials else {
            continue;
        };
        let mut by_item = serde_json::Map::with_capacity(rows.len());
        for (row_index, row) in rows.iter().enumerate() {
            let item_id = row
                .get("item_id")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    format!(
                        "catalog ships[{ship_index}].build_materials[{row_index}] has no item_id"
                    )
                })?;
            let quantity = row.get("quantity").and_then(Value::as_i64).ok_or_else(|| {
                format!(
                    "catalog ships[{ship_index}].build_materials[{row_index}] has no integer quantity"
                )
            })?;
            if by_item
                .insert(item_id.to_string(), Value::from(quantity))
                .is_some()
            {
                return Err(format!(
                    "catalog ships[{ship_index}].build_materials contains duplicate item {item_id}"
                ));
            }
        }
        *materials = Value::Object(by_item);
    }
    Ok(catalog)
}

/// Local typed cache of `/api/map`.
#[derive(Debug, Clone)]
pub struct MapCache {
    map: MapData,
    system_index: HashMap<String, usize>,
}

impl MapCache {
    pub fn new(map: Value) -> Result<Self, String> {
        let map: MapData = serde_json::from_value(map)
            .map_err(|err| format!("map returned invalid data: {err}"))?;
        let system_index = index_ids(map.systems.iter().map(|entry| entry.system_id.as_str()));
        Ok(Self { map, system_index })
    }

    pub async fn load(http_base_url: &str, http: &dyn DataHttpClient) -> Result<Self, String> {
        let data = http
            .get_json(&format!("{}/api/map", trim_trailing_slash(http_base_url)))
            .await?;
        Self::new(data)
    }

    pub fn system(&self, id: &str) -> Option<&MapSystem> {
        self.system_index.get(id).map(|&i| &self.map.systems[i])
    }
    pub fn systems(&self) -> &[MapSystem] {
        &self.map.systems
    }
    pub fn empires(&self) -> &HashMap<String, String> {
        &self.map.empires
    }
    pub fn data(&self) -> &MapData {
        &self.map
    }
}

fn index_ids<'a>(ids: impl Iterator<Item = &'a str>) -> HashMap<String, usize> {
    ids.enumerate()
        .map(|(index, id)| (id.to_owned(), index))
        .collect()
}

pub fn http_base_from_ws(ws_url: &str) -> String {
    let without_scheme = ws_url
        .strip_prefix("wss://")
        .or_else(|| ws_url.strip_prefix("ws://"))
        .unwrap_or(ws_url);
    let authority = without_scheme.split('/').next().unwrap_or(without_scheme);
    let scheme = if ws_url.starts_with("wss://") {
        "https"
    } else {
        "http"
    };
    format!("{scheme}://{authority}")
}

fn trim_trailing_slash(value: &str) -> &str {
    value.trim_end_matches('/')
}
