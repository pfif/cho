use std::fmt::Formatter;
use chrono::NaiveDate;
use serde::{Deserialize, Deserializer};
use serde::de::{Error, Visitor};
use crate::line::LineWithDateVisitor;
use crate::period::Period;

/// A payment is a date and a period id.
#[derive(Clone, Debug)]
pub struct Payment(pub(super) (NaiveDate, Period));

impl<'de> Deserialize<'de> for Payment {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PeriodIDVisitor;
        impl<'de> Visitor<'de> for PeriodIDVisitor {
            type Value = Period;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter.write_str("a period id")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                // TODO rewrite with nom
                let second_char_idx = v
                    .char_indices()
                    .nth(1)
                    .map(|(second_char_idx, _)| second_char_idx)
                    .ok_or_else(|| E::custom("invalid period id: missing space after date"))?;
                Period::try_from(&v[second_char_idx..])
                    .map_err(|err| E::custom(format!("invalid period id: {err}")))
            }
        }

        //                                         Note that the call to the visitor above is indirect!
        let visitor = LineWithDateVisitor::new(PeriodIDVisitor);
        let tuple = deserializer.deserialize_str(visitor)?;
        Ok(Payment(tuple))
    }
}