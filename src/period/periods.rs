use std::fmt::Formatter;
use std::str::FromStr;

use chrono::{Datelike, Months, NaiveDate};
use serde::de::{Error as DeError, Unexpected, Visitor};
use serde::ser::Error as SerError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Period {
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}

impl<'de> Deserialize<'de> for Period {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PeriodVisitor {}
        impl<'de> Visitor<'de> for PeriodVisitor {
            type Value = Period;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter.write_str("An ISO time interval representation ('YYYY-MM-DD/YYYY-MM-DD'), or a month ('YYYY-MM')")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                fn split<'a, E: DeError>(
                    s: &'a str,
                    splitter: &str,
                    exp: &str,
                ) -> Result<(&'a str, &'a str), E> {
                    let split = s.split(splitter).collect::<Vec<&str>>();
                    if split.len() != 2 {
                        return Err(DeError::invalid_value(Unexpected::Str(&s), &exp));
                    }
                    Ok((split[0], split[1]))
                }
                let (start_date, end_date) = if v.len() == 21 {
                    let (raw_start, raw_end) =
                        split(&v, "/", "Two ISO dates separated by a slash")?;
                    fn parse_date<E: DeError>(raw: &str) -> Result<NaiveDate, E> {
                        NaiveDate::parse_from_str(raw, "%F").or(Err(DeError::invalid_value(
                            Unexpected::Str(raw),
                            &"an ISO date",
                        )))
                    }
                    let start_date = parse_date(raw_start)?;
                    let end_date = parse_date(raw_end)?;
                    (start_date, end_date)
                } else if v.len() == 7 {
                    let (raw_year, raw_month) =
                        split(v, "-", "a year and a month separated by a dash")?;
                    fn parse_number<O: FromStr, E: DeError>(raw: &str) -> Result<O, E> {
                        raw.parse::<O>().or_else(|_| {
                            Err(DeError::invalid_value(
                                Unexpected::Str(raw),
                                &"a valid number",
                            ))
                        })
                    }
                    let year = parse_number(raw_year)?;
                    let month = parse_number(raw_month)?;

                    let start_date = NaiveDate::from_ymd_opt(year, month, 1).ok_or(
                        DeError::invalid_value(Unexpected::Str(&v), &"a valid month"),
                    )?;
                    let end_date = (start_date + Months::new(1))
                        .pred_opt()
                        .ok_or(DeError::custom("Could not construct month"))?;
                    (start_date, end_date)
                } else {
                    return Err(DeError::invalid_length(
                        v.len(),
                        &"a either 7 or 21 characters long string",
                    ));
                };
                Ok(Period {
                    start_date,
                    end_date,
                })
            }
        }
        deserializer.deserialize_str(PeriodVisitor {})
    }
}

impl Serialize for Period {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.start_date.day() == 1 {
            let end_of_month = {
                let day_after_end = self
                    .end_date
                    .succ_opt()
                    .ok_or(SerError::custom("Could not compute day after end of month"))?;
                self.end_date.month() != day_after_end.month()
            };
            if end_of_month {
                return serializer
                    .serialize_str(self.start_date.format("%Y-%m").to_string().as_str());
            }
        }

        serializer.serialize_str(
            format!(
                "{}/{}",
                self.start_date.format("%F"),
                self.end_date.format("%F")
            )
            .as_str(),
        )
    }
}

#[cfg(test)]
mod test_deserialize {
    use chrono::NaiveDate;
    use serde_test::{assert_tokens, Token};

    use super::Period;

    #[test]
    fn test_deserialize_precise_period() {
        assert_tokens(
            &Period {
                start_date: NaiveDate::from_ymd_opt(2021, 1, 1).unwrap(),
                end_date: NaiveDate::from_ymd_opt(2021, 1, 28).unwrap(),
            },
            &[Token::Str(&"2021-01-01/2021-01-28")],
        );
    }

    #[test]
    fn test_deserialize_month() {
        assert_tokens(
            &Period {
                start_date: NaiveDate::from_ymd_opt(2021, 1, 1).unwrap(),
                end_date: NaiveDate::from_ymd_opt(2021, 1, 31).unwrap(),
            },
            &[Token::Str(&"2021-01")],
        );
    }
}
