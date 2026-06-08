use chrono::NaiveDate;

/// A structure that holds information garenteed to be in chronological order
pub struct ChronoStack<E: Clone> {
    items: Vec<(NaiveDate, E)>,
}

impl<E: Clone> ChronoStack<E> {
    pub fn new(items: &[(NaiveDate, E)]) -> Result<ChronoStack<E>, String> {
      Self::check_items_in_chronological_order(items)?;
      Ok(ChronoStack { items: items.to_vec() })
    }

  fn check_items_in_chronological_order(items: &[(NaiveDate, E)]) -> Result<(), String> {
    for window in items.windows(2) {
      let (prev_date, _) = &window[0];
      let (next_date, _) = &window[1];
      if next_date < prev_date {
        return Err(format!(
          "items are not in chronological order: {} comes after {}",
          next_date, prev_date
        ))
      }
    };
    Ok(())
  }

  pub fn iter(&self) -> impl Iterator<Item=&(NaiveDate, E)> {
        self.items.iter()
    }
}

#[cfg(test)]
mod tests {
    mod chronological_test {
        use chrono::NaiveDate;
        use super::super::ChronoStack;

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
            let items: Vec<(NaiveDate, i8)> = vec![
                (d(2026, 1, 1), 1),
                (d(2026, 2, 1), 2),
                (d(2026, 3, 1), 3),
            ];
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
            let items: Vec<(NaiveDate, i8)> = vec![
                (d(2026, 2, 1), 1),
                (d(2026, 1, 1), 2),
            ];
            assert!(ChronoStack::new(&items).is_err());
        }

        #[test]
        fn two_items_same_date_is_ok() {
            let items: Vec<(NaiveDate, i8)> = vec![
                (d(2026, 1, 1), 1),
                (d(2026, 1, 1), 2),
            ];
            assert!(ChronoStack::new(&items).is_ok());
        }

        #[test]
        fn last_item_out_of_order_fails() {
            let items: Vec<(NaiveDate, i8)> = vec![
                (d(2026, 1, 1), 1),
                (d(2026, 2, 1), 2),
                (d(2026, 1, 15), 3),
            ];
            assert!(ChronoStack::new(&items).is_err());
        }

        #[test]
        fn first_item_out_of_order_fails() {
            let items: Vec<(NaiveDate, i8)> = vec![
                (d(2026, 3, 1), 1),
                (d(2026, 1, 1), 2),
                (d(2026, 2, 1), 3),
            ];
            assert!(ChronoStack::new(&items).is_err());
        }
    }
}