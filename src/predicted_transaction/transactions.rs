use crate::amounts::{CurrencyIdent, Figure};
use crate::period::Period;

pub struct TransactionList {
    pub amount: Figure,
    pub currency: CurrencyIdent,

    pub occurrences: Vec<Occurrence>,
}

pub struct Occurrence {
    period: Period,
    paid: bool,
}
