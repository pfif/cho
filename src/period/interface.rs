use std::cmp::Ordering;
use std::fmt::{Formatter};
use crate::period::calendar_month_period::{CalendarMonthPeriodConfiguration};
use crate::vault::VaultReadable;
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Deserializer};
use serde::de::Error;
use strum::{EnumIter};

#[derive(Deserialize, EnumIter)]
#[serde(tag = "type")]
pub enum PeriodConfigurationVaultValue {
    #[serde(rename = "monthly")]
    CalendarMonth,
}

impl VaultReadable for PeriodConfigurationVaultValue {
    const KEY: &'static str = "periods_configuration";
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum ErrorPeriodsBetween {
    EndBeforeStart,
    Miscelaneous(String)
}

impl From<ErrorPeriodsBetween> for String {
    fn from(error_periods_between: ErrorPeriodsBetween) -> String {
        match error_periods_between {
            ErrorPeriodsBetween::EndBeforeStart => "End date is before start date".to_string(),
            ErrorPeriodsBetween::Miscelaneous(s) => s.to_string(),
        }
    }
}

pub trait PeriodsConfiguration{
    fn period_for_date(date: &NaiveDate) -> Result<Period, String>;
    fn periods_between_nb(start: &NaiveDate, end: &NaiveDate) -> Result<u16, ErrorPeriodsBetween>;
    fn periods_between(start: &Period, end: &Period) -> Result<Vec<Period>, String> ;
    fn id_for_period(period: &Period) -> Result<String, String>;
    fn period_from_id(value: &str) -> Result<Period, String>;
}
#[derive(Debug, PartialEq, Eq, Clone, Hash, Ord)]
pub struct Period {
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}

impl Period {
    pub fn contains(&self, date: &NaiveDate) -> bool {
        self.start_date <= *date && *date <= self.end_date
    }
}

impl PartialOrd for Period {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.start_date.cmp(&other.start_date))
    }
}

#[cfg(test)]
mod test {
    use chrono::NaiveDate;
    use strum::IntoEnumIterator;
    use crate::period::{CalendarMonthPeriodConfiguration, Period, PeriodConfigurationVaultValue, PeriodsConfiguration};

    #[test]
    fn test() {

        for config in PeriodConfigurationVaultValue::iter() {
            match config {
                CalendarMonthPeriodConfiguration => {
                    test_to_and_from_id::<CalendarMonthPeriodConfiguration>("2023-04", Period {
                        start_date: NaiveDate::from_ymd_opt(2023, 4, 1).expect("Could parse date"),
                        end_date: NaiveDate::from_ymd_opt(2023, 4, 30).expect("Could parse date"),
                    })
                }
            }
        }
    }

    fn test_to_and_from_id<P: PeriodsConfiguration>(period_str_repr: &str, expected_period: Period) {
        let built_period = Period::try_from(period_str_repr).expect("Could parse period");
        assert_eq!(expected_period, built_period);
        assert_eq!(period_str_repr, P::id_for_period(&built_period).expect("Could convert period to string"));
    }
}

impl<'a> TryFrom<&'a str> for Period {
    type Error = String;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        // TODO Right. I should be calling the interface here. This means fairly profound change to
        //      the design, which I will get to later
        CalendarMonthPeriodConfiguration::period_from_id(value)
    }
}

impl<'de> Deserialize<'de> for Period {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>
    {
        struct PeriodVisitor;
        impl<'de> serde::de::Visitor<'de> for PeriodVisitor {
            type Value = Period;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                todo!()
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: Error
            {
                Period::try_from(v).map_err(|e| E::custom(e))
            }
        }

        deserializer.deserialize_str(PeriodVisitor)
    }
}
