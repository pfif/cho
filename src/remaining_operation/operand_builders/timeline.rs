use chrono::NaiveDate;
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

pub trait ProvidesTimelineOperandValues {
    fn provide(
        &self,
        period: &Period,
        today: &NaiveDate,
        exchange_rates: &ExchangeRates,
        // TODO - should we actually be getting references to all these values here, instead of copies? What impact does that have on memory?
    ) -> Result<TimelineOperandValues, String>;
}

pub struct TimelineOperandBuilder<V: ProvidesTimelineOperandValues> {
    pub values_provider: V,
}
impl<V: ProvidesTimelineOperandValues> OperandBuilder for TimelineOperandBuilder<V> {
    fn build(
        &self,
        period: &Period,
        today: &NaiveDate,
        exchange_rates: &ExchangeRates,
    ) -> Result<Operand, String> {
        let values = self.values_provider.provide(period, today, exchange_rates)?;

        let (end_amount, predicted) = match &values.wrapper_end_amount {
            TimelineOperandEnd::Current(amount) => (amount.clone(), false),
            TimelineOperandEnd::Predicted(amount) => (amount.clone(), true)
        };

        let difference = &end_amount - &values.start_amount;

        let mut illustration: Illustration = Vec::new();
        illustration.push(("Period start amount".into(), IllustrationValue::Amount(values.start_amount)));
        illustration.push(("Period end amount".into(), IllustrationValue::Amount(end_amount)));
        illustration.push(("Period end amount predicted".into(), IllustrationValue::Bool(predicted)));
        illustration.push(("Difference".into(), IllustrationValue::Amount(difference.clone())));

        Ok(Operand {
            name: values.name,
            amount: difference,
            illustration,
        })
    }
}