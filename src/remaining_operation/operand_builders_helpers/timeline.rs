use crate::amounts::Amount;
use crate::remaining_operation::core_types::{Illustration, IllustrationValue, Operand};
use chrono::NaiveDate;

pub enum TimelineOperandEnd {
    Current(Amount),
    Predicted(Amount),
}

pub struct TimelineOperandBuilderHelper {
    pub name: String,
    pub start_amount: Amount,
    pub wrapper_end_amount: TimelineOperandEnd,
    pub archived_since: Option<NaiveDate>,
}
impl TimelineOperandBuilderHelper {
    pub fn build(
        self,
    ) -> Result<Vec<Operand>, String> {
        let (end_amount, predicted) = match &self.wrapper_end_amount {
            TimelineOperandEnd::Current(amount) => (amount.clone(), false),
            TimelineOperandEnd::Predicted(amount) => (amount.clone(), true)
        };

        let difference = end_amount.clone() - self.start_amount.clone();

        let mut illustration: Illustration = Vec::new();
        illustration.push(("Period start amount".into(), IllustrationValue::Amount(self.start_amount.clone())));
        illustration.push(("Period end amount".into(), IllustrationValue::Amount(end_amount)));
        illustration.push(("Committed".into(), IllustrationValue::Bool(!predicted)));
        illustration.push(("Difference".into(), IllustrationValue::Amount(difference.clone())));

        Ok(vec![Operand {
            name: self.name.clone(),
            amount: difference,
            illustration,
            archived_from: self.archived_since,
        }])
    }
}