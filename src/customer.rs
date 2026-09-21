use crate::types::CustomerId;

#[derive(Debug, Clone)]
pub struct CustomerAccount {
    pub id: CustomerId,
    pub email: String,
    balance: f64,
}

impl CustomerAccount {
    pub fn new(id: CustomerId, email: impl Into<String>, initial_balance: f64) -> Self {
        Self {
            id,
            email: email.into(),
            balance: initial_balance,
        }
    }

    pub fn balance(&self) -> f64 {
        self.balance
    }

    pub fn deposit(&mut self, amount: f64) {
        if amount > 0.0 {
            self.balance += amount;
        }
    }

    pub fn charge_account(&mut self, amount: f64) -> bool {
        if amount > 0.0 && self.balance >= amount {
            self.balance -= amount;
            true
        } else {
            false
        }
    }
}