use crate::{period::PeriodsConfiguration, vault::Vault};
use chrono::NaiveDate;
#[cfg(test)]
use mockall::automock;
use serde::Deserialize;

pub type Figure = u32;

#[derive(Deserialize)]
pub struct GoalVaultValues {
    pub goals: Vec<GoalImplementation>
}

impl Vault {
    pub fn read_goals(&self) -> Result<GoalVaultValues, String> {
        return self.read_vault_values("goals".into());
    }
}

#[derive(Deserialize)]
pub struct GoalImplementation {
    name: String,
    currency: String,
    target: Figure,
    commited: Vec<(NaiveDate, Figure)>,
    target_date: NaiveDate,
}

#[cfg_attr(test, automock)]
pub trait Goal<P: PeriodsConfiguration> {
    fn name(&self) -> &String;
    fn currency(&self) -> &String;
    fn target(&self) -> &Figure;
    fn commited(&self) -> &Vec<(NaiveDate, Figure)>;
    fn target_date(&self) -> &NaiveDate;

    fn remaining(&self) -> Result<Figure, String>;
    fn to_pay_at(&self, period_config: &P, date: &NaiveDate) -> Result<Figure, String>;
}

impl GoalImplementation{
    fn remaining(&self) -> Result<Figure, String> {
        let total_commited = self
            .commited
            .iter()
            .fold(0, |acc, (_, amount)| acc + amount);
        if total_commited > self.target {
            return Err("Commited above Goal's target".to_string());
        }

        return Ok(self.target - total_commited);
    }
}

impl<P: PeriodsConfiguration> Goal<P> for GoalImplementation {
    fn name(&self) -> &String {
        return &self.name;
    }

    fn currency(&self) -> &String {
        return &self.currency;
    }

    fn target(&self) -> &Figure {
        return &self.target;
    }

    fn commited(&self) -> &Vec<(NaiveDate, Figure)> {
        return &self.commited;
    }

    fn target_date(&self) -> &NaiveDate {
        return &self.target_date;
    }

    fn remaining(&self) -> Result<Figure, String> {
        /* Telling the compiler that, when looking for the
         * implementation of Goal<P>.remaining() for
         * GoalImplementation on any P, it should use GoalImplementation's
         * function directly */
        self.remaining()
    }

    fn to_pay_at(&self, period_config: &P, date: &NaiveDate) -> Result<Figure, String> {
        if date > &self.target_date {
            return self.remaining();
        }
        let current_period = period_config.period_for_date(date)?;

        let mut commits_iter = self.commited.iter();
        if let Some(mut current_commit) = commits_iter.next() {
            loop {
                let Some(next_commit) = commits_iter.next() else {
                    break;
                };

                let current_commit_date = current_commit.0;
                let next_commit_date = next_commit.0;
                if current_commit_date > next_commit_date {
                    return Err(format!(
                        "Goal '{}': Commits should be in chronological order",
                        self.name
                    ));
                }

                current_commit = next_commit;
            }

            let last_commit = current_commit;
            let last_commit_date = &last_commit.0;
            if last_commit_date > date {
                return Err(format!(
                    "Goal '{}': Computing what was to be paid in the past is not supported.",
                    &self.name
                ));
            }

            if last_commit_date >= &current_period.start_date
                && last_commit_date <= &current_period.end_date
            {
                return Ok(0);
            }
        };

        let remaining = self.remaining()?;

        return Ok(remaining / period_config.periods_between(date, &self.target_date)? as u32);
    }
}

#[allow(non_snake_case)]
#[cfg(test)]
mod test_remaining {
    use super::{Figure, Goal, GoalImplementation};
    use chrono::NaiveDate;

    fn make_goal(commited: Vec<(NaiveDate, Figure)>) -> GoalImplementation {
        return GoalImplementation {
            name: "Test goal".to_string(),
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

/* TODO
Thinking back, these tests might be too complex.

Instead of assigning such a complex behavior to the
MockPeriodsConfiguration (which, at this point is almost as complex as
the real thing), I should just dumbly assign values to it, and make sure
the function works with those.*/
#[allow(non_snake_case)]
#[cfg(test)]
mod test_to_pay_at {
    use crate::period::{MockPeriodsConfiguration, Period};
    use mockall::predicate::eq;

    use super::{Figure, Goal, GoalImplementation};
    use chrono::{Datelike, Days, NaiveDate};

    fn make_goal(commited: Vec<(NaiveDate, Figure)>) -> GoalImplementation {
        return GoalImplementation {
            name: "Test goal".to_string(),
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
            _ => panic!("Cannot make a date with this month"),
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
        let period_config = make_period_config(today, date(1, 9));
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
    fn commited_after_date() {
        let today = date(1, 2);
        let goal = make_goal(vec![(date(12, 30), 30), (date(1, 3), 30)]);
        let period_config = make_period_config(today, date(1, 1));
        assert_eq!(
            goal.to_pay_at(&period_config, &today).unwrap_err(),
            "Goal 'Test goal': Computing what was to be paid in the past is not supported."
        );
    }

    #[test]
    fn commited_not_in_chronological_order() {
        let today = date(1, 2);
        let goal = make_goal(vec![(date(12, 30), 30), (date(12, 29), 30)]);
        let period_config = make_period_config(today, date(1, 1));
        assert_eq!(
            goal.to_pay_at(&period_config, &today).unwrap_err(),
            "Goal 'Test goal': Commits should be in chronological order"
        );
    }
}
