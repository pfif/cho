use std::fmt::{format, Display, Formatter};
use crate::period::calendar_month_period::{CalendarMonthPeriodConfiguration};
use crate::period::fixed_length_period::FixedLengthPeriodConfiguration;
use crate::vault::VaultReadable;
use chrono::{Datelike, NaiveDate};
use clap::builder::Str;
#[cfg(test)]
use mockall::automock;
use serde::{Deserialize, Deserializer};
use serde::de::Error;

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum PeriodConfigurationVaultValue {
    #[serde(rename = "fixed_length")]
    FixedLength(FixedLengthPeriodConfiguration),
    #[serde(rename = "monthly")]
    CalendarMonth(CalendarMonthPeriodConfiguration),
}

impl VaultReadable for PeriodConfigurationVaultValue {
    const KEY: &'static str = "periods_configuration";
}

impl PeriodConfigurationVaultValue {
    fn unpack(&self) -> &dyn PeriodsConfiguration {
        match self {
            PeriodConfigurationVaultValue::FixedLength(p) => p,
            PeriodConfigurationVaultValue::CalendarMonth(p) => p,
        }
    }
}

impl PeriodsConfiguration for PeriodConfigurationVaultValue {
    fn period_for_date(&self, date: &NaiveDate) -> Result<Period, String> {
        self.unpack().period_for_date(date)
    }

    fn periods_between_nb(&self, start: &NaiveDate, end: &NaiveDate) -> Result<u16, ErrorPeriodsBetween> {
        self.unpack().periods_between_nb(start, end)
    }

    fn id_for_period(&self, period: &Period) -> Result<String, String> {
        self.unpack().id_for_period(period)
    }

    fn period_from_id(&self, value: &str) -> Result<Period, String> {
        self.unpack().period_from_id(value)
    }

    fn previous_period(&self, period: &Period) -> Result<Period, String> {
        self.unpack().previous_period(period)
    }
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
    fn period_for_date(&self, date: &NaiveDate) -> Result<Period, String>;
    fn periods_between_nb(&self, start: &NaiveDate, end: &NaiveDate) -> Result<u16, ErrorPeriodsBetween>;

    // TODO I would like to avoid passing an incompatible period to this function. The best idea I
    //      have had do from from the chunky chunk of text below is to implement a
    //      struct CheckedPeriod<T: PeriodsConfiguration> {
    //          period: Period
    //          _periods_configuration: PhantomData(T)
    //      }
    //      adding a check(p: Period) -> Result<CheckedPeriod<Self>, String>
    //      and turning this function into id_for_period(&self, period: Checked<Self>) -> Result<String, String>
    //
    fn periods_between(&self, start: &Period, end: &Period) -> Result<Vec<Period>, ErrorPeriodsBetween> {
        todo!("\
Note for next time I opened this file: don't start by solving that.\
Try to write the algorithm in PredictedTransactionTemplate.predicted_transactions with that function\
\
Also, it looks like fixed_length periods are not really useful at all in this program. Even for predicted transactions.\
I think something like CalendarWeeksPeriodConfiguration would be less complex to write and maintain, and be enough for most use cases.\
If I one day come up for a use case for a payment repeating every 19 days starting Tuesday June 22nd, 2027 - maybe I'll rethink this decision.\
In the meantime... let's chuck it?");
        /*
        // The robot found this, that is almost right. It may however not be the most optimized code for all PeriodsConfiguration

        let mut periods = Vec::new();
        let mut current_period = self.period_for_date(start)?;
        while current_period.end_date < *end {
            periods.push(current_period);
            current_period = self.period_for_date(&current_period.end_date)?;
        }
        Ok(periods)
        
         */
    }
    // TODO I don't know if these two function need a self. If they don't that would help quite a bit with keeping the Deserialize trait for Period
    //      I think id_for_period is not required ... as I can bake it in the Period or CheckedPeriod
    fn id_for_period(&self, period: &Period) -> Result<String, String>;
    fn period_from_id(&self, value: &str) -> Result<Period, String>;
    // TODO is this really needed? If all I use it for is one test ...
    fn previous_period(&self, period: &Period) -> Result<Period, String>;
}

// A period is a time interval between two dates.
//
// Note: It became apparent that Periods may need to be used as input for operations that would differ per PeriodsConfiguration operation.
//       For instance, getting the "next" period after the current one
//
// I hesitate between two designs:
// - Periods know their PeriodConfiguration, and the PeriodsConfiguration specific operation live on them. (For instance, period.next())
//   Period configuration are responsible for creating / checking that PeriodsConfiguration associated with them match their model. Therefore, it is impossible to get an incompatible period calling a PeriodConfiguration.
//   As a matter of fact, the PeriodConfiguration does the building at all time (even if it is from a string)
//   The interface is also moderatly nicer (period.next(), as opposed to period_config.next(period);
//
//
//   This looks clean, but in practice, knowing the PeriodConfiguration at build time for a Period
//   is quite a heavy lift.
//
//   What I for sure don't want to do is have the Deserialze trait knows the
//   PeriodConfiguration ahead of time (I am not sure serde supports, or even advises that).
//   This would mean that when we deserialize a an object that specifies both a PeriodsConfiguration
//   and a Period (like PredictedTransaction, ultimately), we need to deserialize the PeriodsConfiguration,
//   and then use that to deserialize the Period. Probably possible, but also insanely complex.
//
//   Alternatively, it supposes to have RawPeriod with deser impl, but that then need to be loaded through the PeriodsConfig
//   A design fairly similar to RawAmounts and Amounts, which I find to be a bit heavy
//
//   I also don't want to be wasteful memory-wise by cloning the PeriodsConfiguration over and over.
//   I would like to store a reference to the PeriodsConfig. That would mean either:
//       - making Period into Period<'p, P: PeriodsConfiguration> and store a &'p PeriodsConfiguration (which has repercussions everywhere in the codebase)
//       - Storing a Rc<PeriodsConfiguration>
//
// - Period don't know their PeriodConfiguration, and we pass them to the PeriodsConfiguration. It's
//   simpler for now, and I am trying to get to a prototype going.
//   But this does have drawbacks:
//       - calls are ugly-ish [period_config.next(period)]
//       - there are Result<> for all these operations, as we need to error if an incompatible Period
//         is passed
//       - the error will be less obvious, as it won't happen when starting to work with the
//         PeriodsConfiguration, but in the middle of its usage
//         (although this could also be achieved with a method on PeriodsConfiguration that checks
//         the date. It's just that we will keep needing to remember to call it. Or maybe this could
//         all be done with a simple Marker on the Period? Or another type called PeriodChecked which
//         calls the Periods. PeriodChecked can only be built by a PeriodsConfiguration. This last one
//         keeps things truly simple
//
//   Let's keep period like this as I keep prototyping, but I feel like refactoring to get Period
//   linked to a PeriodsConfiguration will be in order at some point
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Period {
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}

impl Period {
    pub fn contains(&self, date: &NaiveDate) -> bool {
        self.start_date <= *date && *date <= self.end_date
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn todo_display_for_period(){
        todo!()
    }

    #[test]
    fn todo_id_to_and_from_period(){
        todo!()
    }

    fn todo_previous_and_next_periods(){
        todo!()
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
