//! Generated OpenAPI schema DTOs.

#![allow(clippy::derivable_impls, clippy::large_enum_variant)]

include!(concat!(env!("OUT_DIR"), "/types.gen.rs"));

impl CatalogDumpItemsItem {
    pub fn id(&self) -> &str {
        match self {
            Self::Item(item) => &item.id,
            Self::Module(module) => &module.id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Item(item) => &item.name,
            Self::Module(module) => &module.name,
        }
    }

    pub fn cargo_size(&self) -> Option<i64> {
        match self {
            Self::Item(item) => Some(item.size),
            Self::Module(module) => Some(module.size),
        }
    }

    pub fn module_type(&self) -> Option<&str> {
        match self {
            Self::Item(_) => None,
            Self::Module(module) => Some(&module.type_),
        }
    }
}

impl CatalogResponseItemsItem {
    /// Stable catalog identifier shared by every generated catalog variant.
    pub fn id(&self) -> &str {
        match self {
            Self::Item(value) => &value.id,
            Self::Module(value) => &value.id,
            Self::ShipClass(value) => &value.id,
            Self::Skill(value) => &value.id,
            Self::Recipe(value) => &value.id,
            Self::FacilityDefinition(value) => &value.id,
        }
    }

    /// Display name shared by every generated catalog variant.
    pub fn name(&self) -> &str {
        match self {
            Self::Item(value) => &value.name,
            Self::Module(value) => &value.name,
            Self::ShipClass(value) => &value.name,
            Self::Skill(value) => &value.name,
            Self::Recipe(value) => &value.name,
            Self::FacilityDefinition(value) => &value.name,
        }
    }

    /// Cargo size for item/module variants that occupy cargo space.
    pub fn cargo_size(&self) -> Option<i64> {
        match self {
            Self::Item(value) => Some(value.size),
            Self::Module(value) => Some(value.size),
            _ => None,
        }
    }

    /// Module slot family for module catalog variants.
    pub fn module_type(&self) -> Option<&str> {
        match self {
            Self::Module(value) => Some(&value.type_),
            _ => None,
        }
    }
}
