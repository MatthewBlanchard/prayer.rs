//! Local cache of subscribed station order books using generated DTOs.

use std::collections::HashMap;

use crate::schema::{MarketItemBook, NotificationMarketUpdate, SubscribeMarketResponse};

#[derive(Debug, Clone)]
pub enum MarketItem {
    Snapshot(MarketItemBook),
    Update(MarketItemBook),
}

impl MarketItem {
    pub fn item_id(&self) -> &str {
        match self {
            Self::Snapshot(item) => &item.item_id,
            Self::Update(item) => &item.item_id,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MarketBook {
    pub base_id: String,
    pub base_name: Option<String>,
    pub tick: i64,
    pub items: HashMap<String, MarketItem>,
}

#[derive(Debug, Clone, Default)]
pub struct MarketCache {
    books: HashMap<String, MarketBook>,
}

impl MarketCache {
    pub fn seed(&mut self, snapshot: SubscribeMarketResponse) -> String {
        let base_id = snapshot.base_id;
        let items = snapshot
            .items
            .into_iter()
            .map(|item| (item.item_id.clone(), MarketItem::Snapshot(item)))
            .collect();
        self.books.insert(
            base_id.clone(),
            MarketBook {
                base_id: base_id.clone(),
                base_name: Some(snapshot.base_name),
                tick: 0,
                items,
            },
        );
        base_id
    }

    pub fn apply_update(&mut self, update: NotificationMarketUpdate) {
        let book = self
            .books
            .entry(update.base_id.clone())
            .or_insert_with(|| MarketBook {
                base_id: update.base_id.clone(),
                base_name: None,
                tick: 0,
                items: HashMap::new(),
            });
        book.tick = update.tick;
        if update.base_name.is_some() {
            book.base_name = update.base_name;
        }
        for item in update.items {
            book.items
                .insert(item.item_id.clone(), MarketItem::Update(item));
        }
    }

    pub fn book(&self, base_id: &str) -> Option<&MarketBook> {
        self.books.get(base_id)
    }
    pub fn bases(&self) -> Vec<&str> {
        self.books.keys().map(String::as_str).collect()
    }
    pub fn drop(&mut self, base_id: &str) {
        self.books.remove(base_id);
    }
}
