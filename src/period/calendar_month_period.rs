use chrono::{Datelike, NaiveDate, Months};
use serde::Deserialize;
use crate::period::{Period, PeriodsConfiguration};

#[derive(Deserialize)]
struct CalendarMonthPeriodConfiguration{}

impl PeriodsConfiguration for CalendarMonthPeriodConfiguration{
    fn period_for_date(&self, date: &NaiveDate) -> Result<Period, String> {
        Ok(Period{
            start_date: date.with_day(1).ok_or("Could not compute the first day of the month")?,
            end_date: (|| {
                let next_month = *date + Months::new(1);
                let first_day = next_month.with_day(1)?;
                first_day.pred_opt()
            })().ok_or("Could not compute the last day of the month")?
        })
    }

    fn periods_between(&self, start: &NaiveDate, end: &NaiveDate) -> Result<u16, String> {
        todo!()
    }
}

// TODO: Test 31st

#[cfg(test)]
mod period_for_date_tests{
    use chrono::NaiveDate;
    use derive_builder::Builder;
    use crate::period::calendar_month_period::CalendarMonthPeriodConfiguration;
    use crate::period::{Period, PeriodsConfiguration};
    
    fn date(month: u32, day: u32) -> NaiveDate {
        return NaiveDate::from_ymd_opt(2023, month, day).unwrap();
    }

    #[derive(Builder)]
    #[builder(
    pattern = "immutable",
    build_fn(skip),
    name = "Test")]
    struct Tes{
        input: NaiveDate,
        expected_output: Period
    }
    
    impl Test {
        fn execute(self){
            let config = CalendarMonthPeriodConfiguration{};
            let result = config.period_for_date(&self.input.unwrap()).unwrap();
            assert_eq!(result, self.expected_output.unwrap())
        }
    }
    
    fn thirty_days() -> Test {
        Test::default().expected_output(Period{
            start_date: date(4, 1),
            end_date: date(4, 30)
        })
    }
    
    #[test]
    fn thirty_days__mid_month(){
        thirty_days().input(date(4, 15)).execute();
    }

    #[test]
    fn thirty_days__end_of_month(){
        thirty_days().input(date(4, 30)).execute();
    }

    #[test]
    fn thirty_days__beginning_of_month(){
        thirty_days().input(date(4, 1)).execute();
    }
    fn thirty_one_days() -> Test {
        Test::default().expected_output(Period{
            start_date: date(5, 1),
            end_date: date(5, 31)
        })
    }

    #[test]
    fn thirty_one_days__mid_month(){
        thirty_one_days().input(date(5, 15)).execute();
    }

    #[test]
    fn thirty_one_days__end_of_month(){
        thirty_one_days().input(date(5, 31)).execute();
    }

    #[test]
    fn thirty_one_days__beginning_of_month(){
        thirty_one_days().input(date(5, 1)).execute();
    }
    
    fn end_of_year() -> Test {
        Test::default().expected_output(Period{
            start_date: date(12, 1),
            end_date: date(12, 31)
        })
    }
    
    #[test]
    fn end_of_year__mid_month(){
        end_of_year().input(date(12, 15)).execute();
    }

    #[test]
    fn end_of_year__end_of_month(){
        end_of_year().input(date(12, 31)).execute();
    }

    #[test]
    fn end_of_year__beginning_of_month(){
        end_of_year().input(date(12, 1)).execute();
    }

    fn february_28() -> Test {
        Test::default().expected_output(Period{
            start_date: date(2, 1),
            end_date: date(2, 28)
        })
    }
    
    #[test]
    fn february_28__mid_month(){
        february_28().input(date(2, 15)).execute();
    }

    #[test]
    fn february_28__end_of_month(){
        february_28().input(date(2, 28)).execute();
    }

    #[test]
    fn february_28__beginning_of_month(){
        february_28().input(date(2, 1)).execute();
    }
    
    fn date_bisextile(month: u32, day: u32) -> NaiveDate {
        return NaiveDate::from_ymd_opt(2024, month, day).unwrap();
    }
    
    fn february_29() -> Test {
        Test::default().expected_output(Period{
            start_date: date_bisextile(2, 1),
            end_date: date_bisextile(2, 29)
        })
    }
    
    #[test]
    fn february_29__mid_month(){
        february_29().input(date_bisextile(2, 15)).execute();
    }

    #[test]
    fn february_29__end_of_month(){
        february_29().input(date_bisextile(2, 29)).execute();
    }

    #[test]
    fn february_29__beginning_of_month(){
        february_29().input(date_bisextile(2, 1)).execute();
    }
}