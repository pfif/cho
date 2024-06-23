use std::collections::HashSet;

use serde::Deserialize;

use transactions::TransactionList;

use crate::amounts::{CurrencyIdent, Figure};
use crate::period::Period;
use crate::period::PeriodsConfiguration;

use super::transactions;

#[derive(Deserialize)]
struct RecurringTransactionsConfiguration<T: PeriodsConfiguration> {
    name: String,
    amount: Figure,
    currency: CurrencyIdent,
    when: T,
    paid: HashSet<Period>,
}

impl<T: PeriodsConfiguration> RecurringTransactionsConfiguration<T> {
    fn generate_predicted_transaction_list(self, p: Period) -> TransactionList {
        return TransactionList {
            amount: self.amount,
            currency: self.currency,

            occurrences: vec![],
        };
    }
}
