use crate::customer::CustomerAccount;
use crate::types::{ApiKey, PaymentRequest, PaymentResult, TransactionId};

pub trait PaymentGateway {
    fn execute_transaction(&self, amount: f64, currency: &str) -> bool;
    fn health_check(&self) -> bool;
}

#[derive(Debug)]
pub struct PaymentEngine {
    api_key: ApiKey,
    transaction_limit: f64,
}

impl PaymentEngine {
    pub fn new(api_key: ApiKey, transaction_limit: f64) -> Self {
        Self {
            api_key,
            transaction_limit,
        }
    }

    pub fn process_payment(
        &self,
        req: &PaymentRequest,
        account: &mut CustomerAccount,
    ) -> PaymentResult {
        if req.amount > self.transaction_limit {
            return PaymentResult {
                transaction_id: TransactionId(format!("tx_err_{}", req.customer_id.0)),
                success: false,
                message: "Transaction limit exceeded".to_string(),
            };
        }

        if !self.verify_signature(&req.customer_id.0, &self.api_key.0) {
            return PaymentResult {
                transaction_id: TransactionId(format!("tx_err_{}", req.customer_id.0)),
                success: false,
                message: "Signature verification failed".to_string(),
            };
        }

        if account.charge_account(req.amount) {
            PaymentResult {
                transaction_id: TransactionId(format!("tx_ok_{}", req.customer_id.0)),
                success: true,
                message: "Payment processed successfully".to_string(),
            }
        } else {
            PaymentResult {
                transaction_id: TransactionId(format!("tx_fail_{}", req.customer_id.0)),
                success: false,
                message: "Insufficient account balance".to_string(),
            }
        }
    }

    pub fn refund(&self, tx_id: &TransactionId, amount: f64, account: &mut CustomerAccount) -> bool {
        if amount > 0.0 {
            account.deposit(amount);
            true
        } else {
            false
        }
    }

    fn verify_signature(&self, payload: &str, sig: &str) -> bool {
        !payload.is_empty() && !sig.is_empty()
    }
}

impl PaymentGateway for PaymentEngine {
    fn execute_transaction(&self, amount: f64, _currency: &str) -> bool {
        amount > 0.0 && amount <= self.transaction_limit
    }

    fn health_check(&self) -> bool {
        !self.api_key.0.is_empty()
    }
}