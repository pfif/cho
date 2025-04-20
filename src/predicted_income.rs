use std::collections::HashMap;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;
use crate::period::Period;
use crate::remaining_operation::amounts::exchange_rates::ExchangeRates;
use crate::remaining_operation::core_types::{GroupBuilder, IllustrationValue, Operand, OperandBuilder};
use crate::vault::{Vault, VaultReadable};

pub struct PredictedIncomeBuilder{
    predicted_income: Option<PredictedIncome>,
}

impl PredictedIncomeBuilder {
    pub fn from_vault_value<V: Vault>(should_be_included: bool, vault: &V) -> Result<PredictedIncomeBuilder, String> {
        let predicted_income = if should_be_included {
            Some(PredictedIncome::from_vault(vault)?)
        } else {
            None
        };

        Ok(PredictedIncomeBuilder{predicted_income})
    }
}

impl Into<GroupBuilder> for PredictedIncomeBuilder {
    fn into(self) -> GroupBuilder {
        GroupBuilder {
            name: "Predicted Income".into(),
            operand_factories: match self.predicted_income {
                Some(predicted_income) => vec![Box::from(predicted_income)],
                None => vec![],
            }
        }
    }
}

#[derive(Deserialize)]
pub struct PredictedIncome{
    currency: String,
    figure: Decimal,
}

impl VaultReadable for PredictedIncome {
    const KEY: &'static str = "predicted_income";
}

impl OperandBuilder for PredictedIncome {
    fn build(&self, period: &Period, today: &NaiveDate, exchange_rates: &ExchangeRates) -> Result<Operand, String> {
        // TODO - This illustration might be best as a default illustration?
        let mut illustration: HashMap<String, IllustrationValue> = HashMap::new();
        let amount = exchange_rates.new_amount(&self.currency, self.figure)?;
        illustration.insert("Amount".into(), IllustrationValue::Amount(amount.clone()));

        Ok(Operand{
            name: "Predicted income".to_string(),
            amount,
            illustration,
        })
    }
}
