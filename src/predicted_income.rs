use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;
use crate::period::{Period, PeriodConfigurationVaultValue};
use crate::remaining_operation::amounts::exchange_rates::ExchangeRates;
use crate::remaining_operation::core_types::{GroupBuilder, IllustrationValue, Operand, OperandBuilder};
use crate::vault::{Vault, VaultReadable};


#[derive(Deserialize)]
pub struct PredictedIncome{
    currency: String,
    figure: Decimal,
}

impl VaultReadable for PredictedIncome {
    const KEY: &'static str = "predicted_income";
}

impl OperandBuilder for PredictedIncome {
    fn build(&self, period_config: &PeriodConfigurationVaultValue, today: &NaiveDate, exchange_rates: &ExchangeRates) -> Result<Option<Operand>, String> {
        // TODO - This illustration might be best as a default illustration?
        let mut illustration = Vec::new();
        let amount = exchange_rates.new_amount(&self.currency, self.figure)?;
        illustration.push(("Amount".into(), IllustrationValue::Amount(amount.clone())));

        Ok(Some(Operand{
            name: "Predicted income".to_string(),
            amount,
            illustration,
        }))
    }
}

impl Into<GroupBuilder> for PredictedIncome {
    // TODO - This somehow does not work
    fn into(self) -> GroupBuilder {
        GroupBuilder {
            name: "Predicted Income".into(),
            operand_factories: vec![Box::from(self)],
        }
    }
}
