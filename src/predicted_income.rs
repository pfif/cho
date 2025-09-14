use chrono::NaiveDate;
use derive_builder::Builder;
use rust_decimal::Decimal;
use serde::Deserialize;
use crate::period::{Period, PeriodConfigurationVaultValue};
use crate::remaining_operation::amounts::exchange_rates::ExchangeRates;
use crate::remaining_operation::core_types::{GroupBuilder, IllustrationValue, Operand, OperandBuilder};
use crate::remaining_operation::core_types::group::Group;
use crate::vault::{Vault, VaultReadable};


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
    fn build(self, period_config: &PeriodConfigurationVaultValue, today: &NaiveDate, exchange_rates: &ExchangeRates) -> Result<Option<Operand>, String> {
        // TODO - This illustration might be best as a default illustration?
        let mut illustration = Vec::new();
        let amount = exchange_rates.new_amount(&self.currency, self.figure)?;
        illustration.push(("Amount".into(), IllustrationValue::Amount(amount.clone())));

        Ok(Some(Operand{
            name: "Predicted Income".to_string(),
            amount,
            illustration,
        }))
    }
}

impl GroupBuilder<PredictedIncome> for PredictedIncome {
    fn build(self) -> Result<(String, Vec<PredictedIncome>), String> {
        Ok(("Predicted Income".into(), vec![self]))
    }
}
