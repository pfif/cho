use crate::period::{Period, PeriodsConfiguration};
use chrono::NaiveDate;
use clap::builder::Str;
use std::fmt::Debug;

/// A structure that holds information garenteed to be in chronological order
pub struct ChronoStack<E: Clone + Debug> {
    items: Vec<(NaiveDate, E)>,
}

impl<E: Clone + Debug> ChronoStack<E> {
    pub fn new(items: &[(NaiveDate, E)]) -> Result<ChronoStack<E>, String> {
        Self::check_items_in_chronological_order(items)?;
        Ok(ChronoStack {
            items: items.to_vec(),
        })
    }

    fn check_items_in_chronological_order(items: &[(NaiveDate, E)]) -> Result<(), String> {
        for window in items.windows(2) {
            let (prev_date, _) = &window[0];
            let (next_date, _) = &window[1];
            if next_date < prev_date {
                return Err(format!(
                    "items are not in chronological order: {} comes after {}",
                    next_date, prev_date
                ));
            }
        }
        Ok(())
    }

    pub fn iter(&self) -> impl Iterator<Item = &(NaiveDate, E)> {
        self.items.iter()
    }

    pub fn into_split_for_period_and_date(
        self,
        period: &Period,
        date: &NaiveDate,
    ) -> (Vec<E>, Vec<E>, Vec<E>) {
        let items = self.items;
        let first_in_period_idx = items.
                iter().
                // Split at the first item ...
                position(|item| (item.0 >= period.start_date)).
                // ... or if None was found, the entire vec is before the period start
                unwrap_or(items.len());

        let first_date_after_period_idx = items.
                iter().
                // Split at the first item after the period start ...
                position(|item| &item.0 > date).
                // ... or if None was found, the entire vec is before the period start
                unwrap_or(items.len());

        let before_period_start = &items[..first_in_period_idx];
        let in_period_before_date = &items[first_in_period_idx..first_date_after_period_idx];
        let after_date = &items[first_date_after_period_idx..];

        (
            before_period_start.iter().map(|(_date, element)| element.clone()).collect(),
            in_period_before_date.iter().map(|(_date, element)| element.clone()).collect(),
            after_date.iter().map(|(_date, element)| element.clone()).collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    mod chronological_test {
        use super::super::ChronoStack;
        use chrono::NaiveDate;

        fn d(y: i32, m: u32, d: u32) -> NaiveDate {
            NaiveDate::from_ymd_opt(y, m, d).unwrap()
        }

        #[test]
        fn empty_list_is_ok() {
            let items: Vec<(NaiveDate, i8)> = vec![];
            assert!(ChronoStack::new(&items).is_ok());
        }

        #[test]
        fn single_item_is_ok() {
            let items: Vec<(NaiveDate, i8)> = vec![(d(2026, 1, 1), 1)];
            assert!(ChronoStack::new(&items).is_ok());
        }

        #[test]
        fn nominal_chronological_order_is_ok() {
            let items: Vec<(NaiveDate, i8)> =
                vec![(d(2026, 1, 1), 1), (d(2026, 2, 1), 2), (d(2026, 3, 1), 3)];
            assert!(ChronoStack::new(&items).is_ok());
        }

        #[test]
        fn same_date_twice_is_ok() {
            let items: Vec<(NaiveDate, i8)> = vec![
                (d(2026, 1, 1), 1),
                (d(2026, 2, 1), 2),
                (d(2026, 2, 1), 3),
                (d(2026, 3, 1), 4),
            ];
            assert!(ChronoStack::new(&items).is_ok());
        }

        #[test]
        fn out_of_order_entry_in_middle_fails() {
            let items: Vec<(NaiveDate, i8)> = vec![
                (d(2026, 1, 1), 1),
                (d(2026, 3, 1), 2),
                (d(2026, 2, 1), 3),
                (d(2026, 4, 1), 4),
            ];
            assert!(ChronoStack::new(&items).is_err());
        }

        #[test]
        fn out_of_order_entry_in_middle_with_duplicates_fails() {
            let items: Vec<(NaiveDate, i8)> = vec![
                (d(2026, 1, 1), 1),
                (d(2026, 2, 1), 2),
                (d(2026, 2, 1), 3),
                (d(2026, 1, 15), 4),
                (d(2026, 3, 1), 5),
            ];
            assert!(ChronoStack::new(&items).is_err());
        }

        #[test]
        fn two_items_reversed_fails() {
            let items: Vec<(NaiveDate, i8)> = vec![(d(2026, 2, 1), 1), (d(2026, 1, 1), 2)];
            assert!(ChronoStack::new(&items).is_err());
        }

        #[test]
        fn two_items_same_date_is_ok() {
            let items: Vec<(NaiveDate, i8)> = vec![(d(2026, 1, 1), 1), (d(2026, 1, 1), 2)];
            assert!(ChronoStack::new(&items).is_ok());
        }

        #[test]
        fn last_item_out_of_order_fails() {
            let items: Vec<(NaiveDate, i8)> =
                vec![(d(2026, 1, 1), 1), (d(2026, 2, 1), 2), (d(2026, 1, 15), 3)];
            assert!(ChronoStack::new(&items).is_err());
        }

        #[test]
        fn first_item_out_of_order_fails() {
            let items: Vec<(NaiveDate, i8)> =
                vec![(d(2026, 3, 1), 1), (d(2026, 1, 1), 2), (d(2026, 2, 1), 3)];
            assert!(ChronoStack::new(&items).is_err());
        }
    }
    mod iters_for_periods_and_date {
        use super::*;
        #[test]
        fn todo() {
            todo!("find all the edge cases and test them all. one edge case is: date is not between period start and end")
        }
    }
}
