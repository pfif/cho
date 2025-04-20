use chrono::NaiveDate;
use std::collections::HashMap;
use crate::period::Period;
use crate::remaining_operation::amounts::Amount;
use crate::remaining_operation::amounts::exchange_rates::ExchangeRates;
use crate::remaining_operation::core_types::{Illustration, IllustrationValue, Operand, OperandBuilder};

pub enum TimelineOperandEnd {
    Current(Amount),
    Predicted(Amount),
}

pub struct TimelineOperandValues {
    pub name: String,
    pub start_amount: Amount,
    pub wrapper_end_amount: TimelineOperandEnd,
}

pub trait TimelineOperandBuilder {
    fn gather_values(
        &self,
        period: &Period,
        today: &NaiveDate,
        exchange_rates: &ExchangeRates,
        // TODO - should we actually be getting references to all these values here, instead of copies? What impact does that have on memory?
    ) -> Result<TimelineOperandValues, String>;
}

impl OperandBuilder for dyn TimelineOperandBuilder {
    fn build(
        &self,
        period: &Period,
        today: &NaiveDate,
        exchange_rates: &ExchangeRates,
    ) -> Result<Operand, String> {
        let values = self.gather_values(period, today, exchange_rates)?;

        let (end_amount, predicted) = match &values.wrapper_end_amount {
            TimelineOperandEnd::Current(amount) => (amount.clone(), false),
            TimelineOperandEnd::Predicted(amount) => (amount.clone(), true)
        };

        let difference = &end_amount - &values.start_amount;

        let mut illustration: Illustration = HashMap::new();
        illustration.insert("Period start amount".into(), IllustrationValue::Amount(values.start_amount));
        illustration.insert("Period end amount".into(), IllustrationValue::Amount(end_amount));
        illustration.insert("Period end amount predicted".into(), IllustrationValue::Bool(predicted));
        illustration.insert("Difference".into(),  IllustrationValue::Amount(difference.clone()));

        Ok(Operand {
            name: values.name,
            amount: difference,
            illustration,
        })
    }
}