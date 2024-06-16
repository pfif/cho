use crate::amounts::{Currency, Figure};
use crate::period::Period;

pub struct Transaction<'a> {
    pub amount: &'a Figure,
    pub currency: &'a Currency,

    pub occurrences: Vec<Occurrence<'a>>,
}

pub struct Occurrence<'a> {
    period: &'a Period,
    paid: bool,
}
