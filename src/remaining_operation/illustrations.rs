use crate::period::Period;
use crate::remaining_operation::amounts::exchange_rates::ExchangeRates;
use crate::remaining_operation::amounts::Amount;
use crate::remaining_operation::core_types::{Illustration, IllustrationValue, Operand, OperandBuilder};
use chrono::NaiveDate;
use std::collections::HashMap;

enum TimelineOperandEnd {
    Current(Amount),
    Predicted(Amount),
}

pub trait TimelineOperandBuilder {
    fn gather_values(
        &self,
        period: &Period,
        today: &NaiveDate,
        exchange_rates: &ExchangeRates,
    ) -> Result<(String, Amount, TimelineOperandEnd), String>;
}

impl OperandBuilder for dyn TimelineOperandBuilder {
    fn build(
        &self,
        period: &Period,
        today: &NaiveDate,
        exchange_rates: &ExchangeRates,
    ) -> Result<Operand, String> {
        let (name, start_amount, wrapped_end_amount) = self.gather_values(period, today, exchange_rates)?;
        
        let (end_amount, predicted) = match wrapped_end_amount {
            TimelineOperandEnd::Current(amount) => (amount, false),
            TimelineOperandEnd::Predicted(amount) => (amount, true)
        };
        
        let difference = &end_amount - &start_amount;

        let mut illustration: Illustration = HashMap::new();
        illustration.insert("Period start amount".into(), IllustrationValue::Amount(start_amount));
        illustration.insert("Period end amount".into(), IllustrationValue::Amount(end_amount.clone()));
        illustration.insert("Period end amount predicted".into(), IllustrationValue::Bool(predicted));
        illustration.insert("Difference".into(),  IllustrationValue::Amount(difference));

        Ok(Operand {
            name,
            amount: end_amount,
            illustration,
        })
    }
}
