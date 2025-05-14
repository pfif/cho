use crate::vault::VaultReadable;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::Deserialize;
use crate::period::{PeriodConfigurationVaultValue, PeriodsConfiguration};
use crate::remaining_operation::amounts::exchange_rates::ExchangeRates;
use crate::remaining_operation::core_types::{GroupBuilder, IllustrationValue, Operand, OperandBuilder};

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

impl OperandBuilder for IgnoredTransaction {
    fn build(&self, period_configuration: &PeriodConfigurationVaultValue, today: &NaiveDate, exchange_rates: &ExchangeRates) -> Result<Option<Operand>, String> {
        let current_period = period_configuration.period_for_date(today)?;
        if !current_period.contains(&self.date) {
           return Ok(None);
        };
        
        let amount = exchange_rates.new_amount(&self.currency, self.amount)?;
        let (included, operand_amount) = if self.date >= *today {
            (true, amount.clone())
        } else {
            (false, exchange_rates.new_amount(&self.currency, dec![0])?)
        };
        Ok(Some(Operand{
            name: self.name.clone(),
            amount: operand_amount, 
            illustration: vec![
                ("Amount".to_string(), IllustrationValue::Amount(amount)),
                ("Included".to_string(), IllustrationValue::Bool(false)),
                ("Date".to_string(), IllustrationValue::Date(self.date.clone()))
            ]
        }))
    }
}

pub type IgnoredTransactionsVaultValues = Vec<IgnoredTransaction>;
impl VaultReadable for IgnoredTransactionsVaultValues {
    const KEY: &'static str = "ignored_transactions";
}

impl Into<GroupBuilder> for IgnoredTransactionsVaultValues {
    fn into(self) -> GroupBuilder {
        GroupBuilder{
            name: "Ignored transactions".to_string(),
            operand_factories: self.into_iter().map(|ignored_transaction: IgnoredTransaction| Box::from(ignored_transaction) as Box<dyn OperandBuilder>).collect()
        }
    }
}
