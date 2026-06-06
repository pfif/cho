use std::ops::Sub;
use chrono::NaiveDate;
use crate::amounts::exchange_rates::ExchangeRates;
use crate::buckets::Action;
use crate::buckets::aggregated_amounts::AggregatedAmounts;
use crate::chrono_stack::{ChronoStackWalker, IntoChronoStackWalker};

pub struct ComputeAggregatedAmountsInitializer {
    date: NaiveDate,
    ex: ExchangeRates
}

impl ComputeAggregatedAmountsInitializer {
    pub fn new(date: NaiveDate, ex: ExchangeRates) -> Self {
        ComputeAggregatedAmountsInitializer {
            date,
            ex,
        }
    }
}

pub struct ComputeAggregatedAmounts {
    aggregated_amounts: AggregatedAmounts,
    at_date_amounts: AtDate<AggregatedAmounts>,
}

impl IntoChronoStackWalker<Action, AggregatedAmounts, ComputeAggregatedAmounts> for ComputeAggregatedAmountsInitializer {
    fn into_chrono_stack_walker(self, date: &NaiveDate, element: &Action) -> Result<ComputeAggregatedAmounts, String> {
        let mut aggregated_amounts = AggregatedAmounts::new(&self.ex)?;
        aggregated_amounts.apply(element)?;

        Ok(ComputeAggregatedAmounts{
            at_date_amounts: self.date.into_chrono_stack_walker(date, &aggregated_amounts)?,
            aggregated_amounts,
        })
    }
}

impl ChronoStackWalker<Action, AggregatedAmounts> for ComputeAggregatedAmounts{
    fn try_visit(&mut self, date: &NaiveDate, element: &Action) -> Result<(), String> {
       self.aggregated_amounts.apply(element)?;
       self.at_date_amounts.try_visit(date, &self.aggregated_amounts)?;
        Ok(())
    }

    fn into_output(self) -> AggregatedAmounts {
        self.at_date_amounts.into_output()
    }
}

pub struct AtDate<T> {
    element: T,
    date: NaiveDate,
}

impl<T: Clone> IntoChronoStackWalker<T, T, AtDate<T>> for NaiveDate {
    fn into_chrono_stack_walker(self, date: &NaiveDate, element: &T) -> Result<AtDate<T>, String> {
        Ok(AtDate{
            element: element.clone(),
            date: self.clone(),
        })
    }
}

impl<T: Clone> ChronoStackWalker<T, T> for AtDate<T> {
    fn try_visit(&mut self, date: &NaiveDate, element: &T) -> Result<(), String> {
        if date <= &self.date {
            self.element = element.clone();
        }
        Ok(())
    }

    fn into_output(self) -> T {
        self.element
    }
}

#[cfg(test)]
mod test {
    use chrono::{Days, NaiveDate};
    use crate::chrono_stack::{ChronoStack, IntoChronoStackWalker};

    #[test]
    fn at_date_test() {
        let date = NaiveDate::from_ymd_opt(1792, 8, 15).expect("can construct the date (around when unfortunate Darney makes an unfortunate French decision)");

        let history = vec![
            (date - Days::new(1), 1),
            (date.clone(),             2),
            (date + Days::new(1), 3),
        ];

        let chrono_stack = ChronoStack::new(&history).expect("can construct chrono stack (dates are in chronological order)");
        let result = chrono_stack.try_initialize_and_walk(date).expect("can walk chrono stack");

        assert_eq!(result, 2);
    }

    /*#[test]
    fn element_after_date() {
        let date = NaiveDate::from_ymd_opt(1792, 8, 15).expect("can construct the date (around when unfortunate Darney makes an unfortunate French decision)");
        let period = Period {
            start_date: date - Days::new(5),
            end_date: date
        };

        let history = vec![
                (period.start_date - Days::new(2), 1),
                (period.start_date - Days::new(1), 2), // Last before period
                (period.start_date.clone(),             3), // On period start
                (period.start_date + Days::new(1), 4),
                (date.clone(),                          5),
                (date + Days::new(1),              6),
                (period.end_date.clone(),               7), // period ends
                (period.end_date + Days::new(1),   8), // period end date
        ];

       let chrono_stack = ChronoStack::new(&history).expect("can construct chrono stack (dates are in chronological order)");
       let aggregated_amount_saver = AtDate::<i8>::new(period, date);
    }*/

    // test case - date outside of period - fail to initialize
}