#[double]
use crate::period::PeriodsConfiguration;
use chrono::NaiveDate;
use mockall_double::double;

type Amount = u32;

pub trait GoalVaultValues {
    fn goals() -> Vec<Goal>;
}

pub struct Goal {
    currency: String,
    target: Amount,
    commited: Vec<(NaiveDate, Amount)>,
    target_date: NaiveDate,
}

impl Goal {
    pub fn remaining(&self) -> Result<Amount, String> {
        let total_commited = self
            .commited
            .iter()
            .fold(0, |acc, (_, amount)| acc + amount);
        if total_commited > self.target {
            return Err("Commited above Goal's target".to_string());
        }

        return Ok(self.target - total_commited);
    }

    pub fn to_pay_at(
        &self,
        period_config: &PeriodsConfiguration,
        date: &NaiveDate,
    ) -> Result<Amount, String> {
        let current_period = period_config.period_for_date(date)?;
        for commit in &self.commited {
            let date = &commit.0;
            if date >= &current_period.start_date && date <= &current_period.end_date {
                return Ok(0);
            }
        }

        let remaining = self.remaining()?;

        return Ok(remaining / period_config.periods_between(date, &self.target_date)? as u32);
    }
}

#[allow(non_snake_case)]
#[cfg(test)]
mod test_remaining {
    use super::{Amount, Goal};
    use chrono::NaiveDate;

    fn make_goal(commited: Vec<(NaiveDate, Amount)>) -> Goal {
        return Goal {
            currency: "JPY".to_string(),
            target: 100,
            target_date: NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
            commited,
        };
    }
    #[test]
    fn remaining__nothing_commited() {
        let goal = make_goal(vec![]);
        assert_eq!(goal.remaining().unwrap(), 100);
    }

    #[test]
    fn remaining__below_target__commited_once() {
        let goal = make_goal(vec![(NaiveDate::from_ymd_opt(2019, 1, 1).unwrap(), 10)]);
        assert_eq!(goal.remaining().unwrap(), 90);
    }

    #[test]
    fn remaining__below_target__commited_many() {
        let goal = make_goal(vec![
            (NaiveDate::from_ymd_opt(2019, 1, 1).unwrap(), 10),
            (NaiveDate::from_ymd_opt(2019, 2, 1).unwrap(), 10),
            (NaiveDate::from_ymd_opt(2019, 3, 1).unwrap(), 10),
        ]);
        assert_eq!(goal.remaining().unwrap(), 70);
    }

    #[test]
    fn remaining__at_target__commited_once() {
        let goal = make_goal(vec![(NaiveDate::from_ymd_opt(2019, 1, 1).unwrap(), 100)]);
        assert_eq!(goal.remaining().unwrap(), 0);
    }

    #[test]
    fn remaining__at_target__commited_many() {
        let goal = make_goal(vec![
            (NaiveDate::from_ymd_opt(2019, 1, 1).unwrap(), 30),
            (NaiveDate::from_ymd_opt(2019, 2, 1).unwrap(), 30),
            (NaiveDate::from_ymd_opt(2019, 3, 1).unwrap(), 40),
        ]);
        assert_eq!(goal.remaining().unwrap(), 0);
    }

    #[test]
    fn remaining__above_target__commited_once() {
        let goal = make_goal(vec![(NaiveDate::from_ymd_opt(2019, 1, 1).unwrap(), 110)]);
        assert_eq!(
            goal.remaining().unwrap_err(),
            "Commited above Goal's target"
        );
    }

    #[test]
    fn remaining__above_target__commited_many() {
        let goal = make_goal(vec![
            (NaiveDate::from_ymd_opt(2019, 1, 1).unwrap(), 60),
            (NaiveDate::from_ymd_opt(2019, 1, 1).unwrap(), 50),
        ]);
        assert_eq!(
            goal.remaining().unwrap_err(),
            "Commited above Goal's target"
        );
    }
}

#[allow(non_snake_case)]
#[cfg(test)]
mod test_to_pay_at {
    use crate::period::{MockPeriodsConfiguration, Period, PeriodNumber};
    use mockall::predicate::eq;

    use super::{Amount, Goal};
    use chrono::{Datelike, Days, NaiveDate};

    fn make_goal(commited: Vec<(NaiveDate, Amount)>) -> Goal {
        return Goal {
            currency: "JPY".to_string(),
            target: 100,
            target_date: date(1, 7),
            commited,
        };
    }

    fn date(month: u32, day: u32) -> NaiveDate {
        let year = match month {
            12 => 2019,
            1 => 2020,
            _ => panic!("Cannot make a date with this month")
        };
        return NaiveDate::from_ymd_opt(year, month, day).unwrap();
    }

    /*
    For these tests, we use the following period configuration:
    December 28th, 2019 - Period start
    December 29th, 2019 -
    December 30th, 2019 -
    December 31st, 2019 -
    January 1st, 2020 - Period start
    January 2nd, 2020 -
    January 3st, 2020 -
    January 4th, 2020 -
    January 5th, 2020 - Period start
    January 6th, 2020 -
    January 7th, 2020 - Goal date
    January 8th, 2020 -
    January 9th, 2020 - Period start
    January 10th, 2020 -
    January 11th, 2020 -
    January 12th, 2020 -
    January 13th, 2020 -
    */

    fn make_period_config(
        passed_current_date: NaiveDate,
        returned_current_period_start: NaiveDate,
    ) -> MockPeriodsConfiguration {
        let mut mock = MockPeriodsConfiguration::new();

        let period = Period {
            start_date: returned_current_period_start,
            end_date: returned_current_period_start + Days::new(4),
        };

        mock.expect_period_for_date()
            .with(eq(passed_current_date))
            .return_const(Ok(period));

        let returned_number_of_period = match returned_current_period_start.day() {
            28 => Ok(3),
            1 => Ok(2),
            5 => Ok(1),
            9 => Err("Period after last, the query would make no sense".to_string()),
            _ => panic!("Unexpected date passed to make_period_config"),
        };

        mock.expect_periods_between()
            .with(eq(passed_current_date), eq(date(1, 7)))
            .return_const(returned_number_of_period);

        return mock;
    }

    #[test]
    fn payed_this_period__nothing_else_commited() {
        let today = date(1, 2);
        let goal = make_goal(vec![(today, 10)]);
        let period_config = make_period_config(today, date(1, 1));
        assert_eq!(goal.to_pay_at(&period_config, &today).unwrap(), 0);
    }

    #[test]
    fn payed_this_period__something_else_commited() {
        let today = date(1, 2);
        let goal = make_goal(vec![(date(12, 29), 10), (today, 10)]);
        let period_config = make_period_config(today, date(1, 1));
        assert_eq!(goal.to_pay_at(&period_config, &today).unwrap(), 0);
    }

    #[test]
    fn nothing_commited() {
        let today = date(1, 2);
        let goal = make_goal(vec![]);
        let period_config = make_period_config(today, date(1, 1));
        assert_eq!(goal.to_pay_at(&period_config, &today).unwrap(), 50);
    }

    #[test]
    fn all_commited__last_period() {
        let today = date(1, 6);
        let goal = make_goal(vec![(date(12, 29), 50), (date(1, 2), 50)]);
        let period_config = make_period_config(today, date(1, 5));
        assert_eq!(goal.to_pay_at(&period_config, &today).unwrap(), 0);
    }

    #[test]
    fn all_commited__after_last_period() {
        let today = date(1, 10);
        let goal = make_goal(vec![(date(12, 29), 50), (date(1, 2), 50)]);
        let period_config = make_period_config(today, date(1, 8));
        assert_eq!(goal.to_pay_at(&period_config, &today).unwrap(), 0);
    }

    #[test]
    fn all_commited__several_periods_left() {
        let today = date(1, 2);
        let goal = make_goal(vec![(date(12, 29), 100)]);
        let period_config = make_period_config(today, date(1, 1));
        assert_eq!(goal.to_pay_at(&period_config, &today).unwrap(), 0);
    }

    #[test]
    fn nothing_commited__last_period() {
        let today = date(1, 6);
        let goal = make_goal(vec![]);
        let period_config = make_period_config(today, date(1, 5));
        assert_eq!(goal.to_pay_at(&period_config, &today).unwrap(), 100);
    }

    #[test]
    fn nothing_commited__after_last_period() {
        let today = date(1, 10);
        let goal = make_goal(vec![]);
        let period_config = make_period_config(today, date(1, 9));
        assert_eq!(goal.to_pay_at(&period_config, &today).unwrap(), 100);
    }

    #[test]
    fn nothing_commited__several_periods_left() {
        let today = date(1, 2);
        let goal = make_goal(vec![]);
        let period_config = make_period_config(today, date(1, 1));
        assert_eq!(goal.to_pay_at(&period_config, &today).unwrap(), 50);
    }

    #[test]
    fn some_amount_commited__last_period() {
        let today = date(1, 6);
        let goal = make_goal(vec![(date(12, 29), 30), (date(1, 3), 20)]);
        let period_config = make_period_config(today, date(1, 5));
        assert_eq!(goal.to_pay_at(&period_config, &today).unwrap(), 50);
    }

    #[test]
    fn some_amount_commited__after_last_period() {
        let today = date(1, 10);
        let goal = make_goal(vec![(date(12, 29), 30), (date(1, 3), 20)]);
        let period_config = make_period_config(today, date(1, 9));
        assert_eq!(goal.to_pay_at(&period_config, &today).unwrap(), 50);
    }

    #[test]
    fn some_amount_commited__several_periods_left() {
        let today = date(1, 2);
        let goal = make_goal(vec![(date(12, 29), 30)]);
        let period_config = make_period_config(today, date(1, 1));
        assert_eq!(goal.to_pay_at(&period_config, &today).unwrap(), 35);
    }

    #[test]
    fn periodconfiguration_fails__period_for_date() {
        panic!("AAAAAAAAAH IMPLEMENT ME")
    }

    #[test]
    fn periodconfiguration_fails__periods_between() {
        panic!("AAAAAAAAAH IMPLEMENT ME")
    }
}
