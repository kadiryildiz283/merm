use std::collections::HashMap;
use crate::types::{ItemList, ReservationTicket, Sku};

/// `InventoryManager` stok seviyelerini yöneten ve rezervasyonları
/// idare eden temel Aggregate yapısıdır.
#[derive(Debug, Default, Clone)]
pub struct InventoryManager {
    stock_levels: HashMap<Sku, u32>,
}

impl InventoryManager {
    /// Yeni ve boş bir `InventoryManager` örneği oluşturur.
    pub fn new() -> Self {
        Self {
            stock_levels: HashMap::new(),
        }
    }

    /// Belirtilen ürün listesini rezerve eder.
    pub fn reserve_stock(&mut self, _items: ItemList) -> ReservationTicket {
        // Rezervasyon mantığı
        ReservationTicket::default()
    }

    /// Önceden alınmış bir rezervasyon biletini serbest bırakır.
    pub fn release_stock(&mut self, _ticket: ReservationTicket) {
        // İptal ve stok iade mantığı
    }

    /// Verilen SKU için stok miktarını artırır.
    pub fn restock(&mut self, sku: Sku, quantity: u32) {
        *self.stock_levels.entry(sku).or_insert(0) += quantity;
    }

    /// Konsola "merhaba" yazdıran ve selamlama metnini döndüren fonksiyon.
    pub fn merhaba(&self) -> &'static str {
        println!("merhaba");
        "merhaba"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merhaba_returns_and_prints() {
        let manager = InventoryManager::new();
        assert_eq!(manager.merhaba(), "merhaba");
    }
}