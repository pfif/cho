use chrono::NaiveDate;
use crate::period::{Period, PeriodConfigurationVaultValue};
use crate::remaining_operation::amounts::Amount;
use crate::remaining_operation::amounts::exchange_rates::ExchangeRates;
use crate::remaining_operation::core_types::{Illustration, IllustrationValue, Operand, OperandBuilder};

pub enum TimelineOperandEnd {
    Current(Amount),
    Predicted(Amount),
}

pub struct TimelineOperandBuilder {
    pub name: String,
    pub start_amount: Amount,
    pub wrapper_end_amount: TimelineOperandEnd,
}
impl OperandBuilder for TimelineOperandBuilder {
    // TODO this does not need to be a OperandBuilder anymore
    fn build(
        self,
        period: &PeriodConfigurationVaultValue,
        today: &NaiveDate,
        exchange_rates: &ExchangeRates,
    ) -> Result<Option<Operand>, String> {
        let (end_amount, predicted) = match &self.wrapper_end_amount {
            TimelineOperandEnd::Current(amount) => (amount.clone(), false),
            TimelineOperandEnd::Predicted(amount) => (amount.clone(), true)
        };

        let difference = &end_amount - &self.start_amount;

        let mut illustration: Illustration = Vec::new();
        illustration.push(("Period start amount".into(), IllustrationValue::Amount(self.start_amount.clone())));
        illustration.push(("Period end amount".into(), IllustrationValue::Amount(end_amount)));
        illustration.push(("Committed".into(), IllustrationValue::Bool(!predicted)));
        illustration.push(("Difference".into(), IllustrationValue::Amount(difference.clone())));

        Ok(Some(Operand {
            name: self.name.clone(),
            amount: difference,
            illustration,
        }))
    }
}