use crate::period::Period;
use crate::remaining_operation::amounts::exchange_rates::ExchangeRates;
use crate::remaining_operation::amounts::Amount;
use crate::remaining_operation::core_types::{Operand, OperandBuilder};
use chrono::NaiveDate;
use std::collections::HashMap;

enum TimelineOperandEnd {
    Current(Amount),
    Predicted(Amount),
}

trait TimelineOperandBuilder {
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
        
        // TODO Correct this - the model does not allow entries in illustrations to have different columns 
        let (end_amount, end_amount_description) = match wrapped_end_amount {
            TimelineOperandEnd::Current(amount) => (amount, "Current amount"),
            TimelineOperandEnd::Predicted(amount) => (amount,"End of period predicted amount")
        };
        
        let difference = &end_amount - &start_amount;

        let mut illustration: HashMap<String, Amount> = HashMap::new();
        illustration.insert("Period start amount".into(), start_amount);
        illustration.insert(end_amount_description.into(), end_amount.clone());


        illustration.insert("Difference".into(),  difference);

        Ok(Operand {
            name,
            amount: end_amount,
            illustration,
        })
    }
}
