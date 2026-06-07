use chrono::NaiveDate;

/// A structure that holds information garenteed to be in chronological order
pub struct ChronoStack<E: Clone> {
    items: Vec<(NaiveDate, E)>,
}

impl<E: Clone> IntoIterator for ChronoStack<E> {
    type Item = (NaiveDate, E);
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<E: Clone> ChronoStack<E> {
    pub fn new(items: &[(NaiveDate, E)]) -> Result<ChronoStack<E>, String> {
        // TODO check choronological order
        Ok(ChronoStack { items: items.to_vec() })
    }

    pub fn try_initialize_and_walk<O, W: ChronoStackWalker<E, O>>(&self, initializer: impl IntoChronoStackWalker<E, O, W>) -> Result<O, String> {
        let mut iterator = self.items.iter();
        let (first_date, first_element) = iterator.next().ok_or("ChronoStackWalker initialized without an element")?;
        let mut walker: W = initializer.into_chrono_stack_walker(first_date, first_element)?;
        iterator
            .map(|(date, element)| walker.try_visit(date, element))
            .collect::<Result<(), String>>()?;
        Ok(walker.into_output())
    }

    pub fn try_walk<O>(&self, walker: impl ChronoStackWalker<E, O>) -> Result<O, String> {
        let mut walker  = walker;
        self.items
            .iter()
            .map(|(date, element)| walker.try_visit(date, element))
            .collect::<Result<(), String>>()?;
        Ok(walker.into_output())
    }
}

pub trait IntoChronoStackWalker<E, O, R: ChronoStackWalker<E, O>> {
    fn into_chrono_stack_walker(self, date: &NaiveDate, element: &E) -> Result<R, String>;
}

pub trait ChronoStackWalker<E, O> {
    fn try_visit(&mut self, date: &NaiveDate, element: &E) -> Result<(), String>;
    fn into_output(self) -> O;
}

#[cfg(test)]
mod tests {
    mod chronological_test {
        #[test]
        fn todo() {
            todo!()
        }
    }
    mod integration_test{
        use chrono::{Datelike, Days, NaiveDate};
        use crate::chrono_stack::{ChronoStack, ChronoStackWalker};

        #[test]
        fn test_chrono_stack_walker() {
            struct ComputeTotalCharacterLength {
                day_of_month_total: u32,
                running_total: usize
            }

            impl ComputeTotalCharacterLength {
                fn new() -> Self{
                    Self{day_of_month_total:0, running_total:0}
                }
            }

            impl ChronoStackWalker<String, (u32, usize)> for ComputeTotalCharacterLength {
                fn try_visit(&mut self, date: &NaiveDate, element: &String) -> Result<(), String> {
                    self.day_of_month_total += date.day();
                    self.running_total += element.chars().count();
                    Ok(())
                }

                fn into_output(self) -> (u32, usize) {
                    (self.day_of_month_total, self.running_total)
                }
            }

            let start_date = NaiveDate::from_ymd_opt(2026, 5, 3).expect("Can create first date");
            let second_date = start_date.checked_add_days(Days::new(1)).expect("Can create second date");

            let word = "Banana".to_string();

            let stack = ChronoStack::new(&vec![
                (start_date, "Banana".to_string()),
                (second_date, "Banana".to_string()),
            ]).expect("Can create stack");

            assert_eq!(
                stack.try_walk(ComputeTotalCharacterLength::new()).expect("Walked without errors"),
                (
                    start_date.day() + second_date.day(),
                    word.len() + word.len()
                )
            );
        }

        fn test_chrono_stack_walker_with_initializer() {
            todo!()
        }
    }
}