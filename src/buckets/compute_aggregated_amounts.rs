use chrono::NaiveDate;
use serde::forward_to_deserialize_any;
use crate::amounts::exchange_rates::ExchangeRates;
use crate::buckets::Action;
use crate::buckets::aggregated_amounts::AggregatedAmounts;
use crate::chrono_stack::ChronoStackWalker;

pub struct ComputeAggregatedAmounts<D: DateGate>{
    totals_computer: AggregatedAmounts,
    date_gate: D
}

impl<D: DateGate> ComputeAggregatedAmounts<D> {
    pub fn new(ex: &ExchangeRates, date_gate: D) -> Result<ComputeAggregatedAmounts<D>, String> {
        Ok(ComputeAggregatedAmounts {
            totals_computer: AggregatedAmounts::new(ex)?,
            date_gate
        })
    }
}

impl<D: DateGate> ChronoStackWalker<Action, AggregatedAmounts> for ComputeAggregatedAmounts<D>{
    fn try_visit(&mut self, date: &NaiveDate, element: &Action) -> Result<(), String> {
        if self.date_gate.is_date_allowed(date) {
            self.totals_computer.apply(element)?;
        }

        Ok(())
    }

    fn into_output(self) -> AggregatedAmounts {
        self.totals_computer
    }
}

trait DateGate {
    fn is_date_allowed(&self, date: &chrono::NaiveDate) -> bool;
}

pub struct UntilDateGate {
    date: NaiveDate
}

impl UntilDateGate { pub fn new(date: &NaiveDate) -> UntilDateGate { UntilDateGate { date: date.clone() } }
}

impl DateGate for UntilDateGate {
    fn is_date_allowed(&self, date: &NaiveDate) -> bool {
        date <= &self.date
    }
}