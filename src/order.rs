use crate::customer::CustomerAccount;
use crate::inventory::InventoryManager;
use crate::notification::NotificationHub;
use crate::payment::PaymentEngine;
use crate::types::{
    CustomerId, ItemList, OrderId, OrderRequest, OrderResponse, OrderStatus, PaymentRequest,
    ReservationTicket,
};

#[derive(Debug)]
pub struct OrderService {
    order_id: OrderId,
    customer_id: CustomerId,
    status: OrderStatus,
    active_reservation: Option<ReservationTicket>,
}

impl OrderService {
    pub fn new(order_id: OrderId, customer_id: CustomerId) -> Self {
        Self {
            order_id,
            customer_id,
            status: OrderStatus::Pending,
            active_reservation: None,
        }
    }

    pub fn order_id(&self) -> &OrderId {
        &self.order_id
    }

    pub fn status(&self) -> OrderStatus {
        self.status
    }

    pub fn create_order(
        &mut self,
        request: OrderRequest,
        payment_engine: &PaymentEngine,
        account: &mut CustomerAccount,
        inventory: &mut InventoryManager,
        notifications: &NotificationHub,
    ) -> Result<OrderResponse, String> {
        if !self.validate_inventory(&request.items, inventory) {
            return Err("Inventory validation failed".to_string());
        }

        let ticket = inventory.reserve_stock(&request.items)?;
        self.active_reservation = Some(ticket);

        let pay_req = PaymentRequest {
            customer_id: request.customer_id.clone(),
            amount: request.total_amount,
            currency: "USD".to_string(),
        };

        let pay_res = payment_engine.process_payment(&pay_req, account);
        if !pay_res.success {
            if let Some(ticket) = self.active_reservation.take() {
                inventory.release_stock(&ticket);
            }
            self.status = OrderStatus::Cancelled;
            return Err(format!("Payment failed: {}", pay_res.message));
        }

        self.status = OrderStatus::Confirmed;

        notifications.send_email(
            &account.email,
            &format!("Order {} confirmed. Total: ${:.2}", self.order_id.0, request.total_amount),
        );

        Ok(OrderResponse {
            order_id: self.order_id.clone(),
            status: self.status,
            total_amount: request.total_amount,
        })
    }

    pub fn cancel_order(
        &mut self,
        id: &OrderId,
        reason: &str,
        inventory: &mut InventoryManager,
        notifications: &NotificationHub,
        customer_email: &str,
    ) -> bool {
        if &self.order_id != id || self.status == OrderStatus::Cancelled {
            return false;
        }

        if let Some(ticket) = self.active_reservation.take() {
            inventory.release_stock(&ticket);
        }

        self.status = OrderStatus::Cancelled;
        notifications.send_email(
            customer_email,
            &format!("Order {} cancelled. Reason: {}", self.order_id.0, reason),
        );
        true
    }

    fn validate_inventory(&self, items: &ItemList, inventory: &InventoryManager) -> bool {
        for (sku, qty) in &items.items {
            if inventory.stock_level(sku) < *qty {
                return false;
            }
        }
        true
    }
}