use crate::vault::VaultReadable;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;

pub type Figure = Decimal;
pub type Currency = String;

#[cfg_attr(test, derive(Clone))]
#[derive(Deserialize)]
pub struct IgnoredTransaction {
    pub name: String,
    pub currency: Currency,
    pub amount: Figure,
    pub date: NaiveDate,
}

pub type IgnoredTransactionsVaultValues = Vec<IgnoredTransaction>;
impl VaultReadable for IgnoredTransactionsVaultValues {
    const KEY: &'static str = "ignored_transactions";
}
