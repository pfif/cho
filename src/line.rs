use std::fmt::Formatter;
use chrono::NaiveDate;
use serde::de::{Error, Visitor};

pub struct LineWithDateVisitor<O> {
    pub other_visitor: O
}

impl<O> LineWithDateVisitor<O> {
    pub fn new(other_visitor: O) -> Self {
        Self { other_visitor }
    }
}

impl<'de, O: Visitor<'de>> Visitor<'de> for LineWithDateVisitor<O> {
    type Value = (NaiveDate, O::Value);

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        formatter.write_str("a line with a date and ")?;
        self.other_visitor.expecting(formatter)
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: Error
    {
        // TODO rewrite with nom?
        let first_space = v.find(' ').ok_or(Error::custom("Could not find first space"))?;
        let raw_date = &v[..first_space];
        let date = NaiveDate::parse_from_str(raw_date, "%Y/%m/%d").map_err(|err| {
            Error::custom(format!(
                "Failed to parse date: {}. Error: {}",
                raw_date, err
            ))
        })?;
        let raw_rest = &v[first_space..];
        let parsed_rest = self.other_visitor.visit_str(raw_rest)?;
        Ok((date, parsed_rest))
    }
}