use std::collections::HashSet;

use serde::Deserialize;

use transactions::Transaction;

use crate::amounts::{Currency, Figure};
use crate::period::Period;
use crate::period::PeriodsConfiguration;

use super::transactions;

#[derive(Deserialize)]
struct RecurringTransactionsConfiguration<T: PeriodsConfiguration> {
    name: String,
    amount: Figure,
    currency: Currency,
    when: T,
    paid: HashSet<Period>,
}

impl<T: PeriodsConfiguration> RecurringTransactionsConfiguration<T> {
    fn generate_predicted_transaction(&self, p: Period) -> Transaction {
        return Transaction {
            amount: &self.amount,
            currency: &self.currency,

            occurrences: vec![],
        };
    }
}
