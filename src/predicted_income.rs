use crate::amounts::exchange_rates::ExchangeRates;
use crate::period::PeriodsConfiguration;
use crate::remaining_operation::core_types::{GroupBuilder, Operand, OperandBuilder};
use crate::vault::VaultReadable;
use chrono::NaiveDate;
use derive_builder::Builder;
use rust_decimal::Decimal;
use serde::Deserialize;


#[cfg_attr(test, derive(Builder))]
#[derive(Deserialize)]
pub struct PredictedIncome{
    currency: String,
    figure: Decimal,
}

impl VaultReadable for PredictedIncome {
    const KEY: &'static str = "predicted_income";
}

impl OperandBuilder for PredictedIncome {
    fn build<P: PeriodsConfiguration>(self, _today: &NaiveDate, exchange_rates: &ExchangeRates) -> Result<Vec<Operand>, String> {
        let amount = exchange_rates.new_amount(&self.currency, self.figure)?;

        Ok(vec![Operand{
            name: "Predicted Income".to_string(),
            amount,
            illustration: Vec::new(),
            archived_from: None,
        }])
    }
}

impl GroupBuilder<PredictedIncome> for PredictedIncome {
    fn build(self) -> Result<(String, Vec<PredictedIncome>), String> {
        Ok(("Predicted Income".into(), vec![self]))
    }
}
