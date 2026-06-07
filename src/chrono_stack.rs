use chrono::NaiveDate;

/// A structure that holds information garenteed to be in chronological order
pub struct ChronoStack<E: Clone> {
    items: Vec<(NaiveDate, E)>,
}

impl<E: Clone> ChronoStack<E> {
    pub fn new(items: &[(NaiveDate, E)]) -> Result<ChronoStack<E>, String> {
        // TODO check choronological order
        Ok(ChronoStack { items: items.to_vec() })
    }

    pub fn iter(&self) -> impl Iterator<Item=&(NaiveDate, E)> {
        self.items.iter()
    }
}

#[cfg(test)]
mod tests {
    mod chronological_test {
        #[test]
        fn todo() {
            todo!()
        }
    }
}