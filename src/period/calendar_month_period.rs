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
        // The number of years between the two dates, including start and end
        let full_years = (end.year() - start.year() + 1) as u16;
        
        let month_to_start = (start.month() - 1) as u16;
        let end_year_end = (12 - end.month()) as u16;
        
        Ok(full_years * 12 - month_to_start - end_year_end)
    }
}

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

#[cfg(test)]
mod test_periods_between {
    use chrono::NaiveDate;
    use crate::period::calendar_month_period::CalendarMonthPeriodConfiguration;
    use crate::period::PeriodsConfiguration;

    fn date(month: u32, day: u32) -> NaiveDate {
        return NaiveDate::from_ymd_opt(2023, month, day).unwrap();
    }

    fn date_next_year(month: u32, day: u32) -> NaiveDate {
        return NaiveDate::from_ymd_opt(2024, month, day).unwrap();
    }
    
    fn date_several_years(month: u32, day: u32) -> NaiveDate {
        return NaiveDate::from_ymd_opt(2026, month, day).unwrap();
    }
    
    struct Test{
        start: NaiveDate,
        end: NaiveDate,
        
        expected_output: u16
    }
    
    impl Test {
        fn execute(&self){
            let config = CalendarMonthPeriodConfiguration{};
            let result = config.periods_between(&self.start, &self.end).unwrap();
            assert_eq!(result, self.expected_output)
        }
    }
    
    #[test]
    fn same_month__ends(){
        Test{start: date(4, 1), end: date(4, 30), expected_output: 1}.execute();
    }

    #[test]
    fn same_month__mid(){
        Test{start: date(4, 4), end: date(4, 15), expected_output: 1}.execute();
    }
    
    #[test]
    fn adjacent_months_ends(){
        Test{start: date(4, 1), end: date(5, 31), expected_output: 2}.execute();
    }

    #[test]
    fn adjacent_months_mid(){
        Test{start: date(4, 4), end: date(5, 15), expected_output: 2}.execute();
    }

    #[test]
    fn adjacent_months__inner_ends(){
        Test{start: date(4, 30), end: date(5, 1), expected_output: 2}.execute();
    }
    
   #[test] 
   fn several_months__ends(){
       Test{start: date(2, 1), end: date(6, 30), expected_output: 5}.execute();
   }

   #[test]
   fn several_months__mid(){
       Test{start: date(2, 26), end: date(6, 15), expected_output: 5}.execute();
   }

   #[test]
   fn several_months__inner_ends(){
       Test{start: date(2, 28), end: date(6, 1), expected_output: 5}.execute();
   }

    #[test]
    fn adjacent_years__ends(){
        Test{start: date(1, 1), end: date_next_year(12, 31), expected_output: 24}.execute();
    }
    
    #[test]
    fn adjacent_years__mid(){
        Test{start: date(10, 17), end: date_next_year(2, 14), expected_output: 5}.execute();
    }
    
    #[test]
    fn adjacent_years__inner_ends(){
        Test{start: date(12, 31), end: date_next_year(1, 1), expected_output: 2}.execute();
    }
    
    #[test]
    fn several_years__ends(){
        Test{start: date(1, 1), end: date_several_years(12, 31), expected_output: 48}.execute();
    }

    #[test]
    fn several_years__mid(){
        // Full years: 2024, 2025 -> 24 months
        // Start year (2023): 3 months
        // End year (2026): 2 months
        // Total: 29 months
        Test{start: date(10, 17), end: date_several_years(2, 15), expected_output: 29}.execute();
    }

    #[test]
    fn several_years__inner_ends(){
        // Full years: 2024, 2025 -> 24 months
        // Start year (2023): 1 month
        // End year (2026): 1 months
        // Total: 26 months
        Test{start: date(12, 31), end: date_several_years(1, 1), expected_output: 26}.execute();
    }
}