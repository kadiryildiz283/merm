use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OrderId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CustomerId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TransactionId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Sku(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiKey(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderStatus {
    Pending,
    Confirmed,
    Cancelled,
    Fulfilled,
}

#[derive(Debug, Clone)]
pub struct ItemList {
    pub items: Vec<(Sku, u32)>,
}

#[derive(Debug, Clone)]
pub struct OrderRequest {
    pub customer_id: CustomerId,
    pub items: ItemList,
    pub total_amount: f64,
}

#[derive(Debug, Clone)]
pub struct OrderResponse {
    pub order_id: OrderId,
    pub status: OrderStatus,
    pub total_amount: f64,
}

#[derive(Debug, Clone)]
pub struct PaymentRequest {
    pub customer_id: CustomerId,
    pub amount: f64,
    pub currency: String,
}

#[derive(Debug, Clone)]
pub struct PaymentResult {
    pub transaction_id: TransactionId,
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct ReservationTicket {
    pub ticket_id: String,
    pub reserved_items: HashMap<Sku, u32>,
}