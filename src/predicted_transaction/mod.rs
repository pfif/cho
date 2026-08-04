use crate::amounts::exchange_rates::ExchangeRates;
use crate::amounts::RawAmount;
use crate::period::{CalendarMonthPeriodConfiguration, Period, PeriodsConfiguration};
use crate::remaining_operation::core_types::{GroupBuilder, OperandBuilder};
use crate::vault::VaultReadable;
use chrono::NaiveDate;
use serde::de::{Error, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt::Debug;
use template::PredictedTransactionTemplate;

pub mod template;
pub mod transaction;

pub type PeriodTransactionsVaultValue = Vec<PredictedTransactionTemplate>;
impl VaultReadable for PeriodTransactionsVaultValue {
    const KEY: &'static str = "predicted_transactions";
}

