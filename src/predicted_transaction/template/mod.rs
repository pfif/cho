use std::collections::HashSet;
use chrono::NaiveDate;
use serde::{Deserialize, Deserializer};
use serde::de::{Error, Visitor};
use raw_transaction::RawTransaction;
use crate::amounts::exchange_rates::ExchangeRates;
use crate::amounts::RawAmount;
use crate::chrono_stack::ChronoStack;
use crate::period::{Period, PeriodsConfiguration};
use crate::predicted_transaction::PredictedTransactionsVaultValue;
use crate::predicted_transaction::transaction::PredictedTransaction;
use crate::remaining_operation::core_types::{GroupBuilder, Operand, OperandBuilder};

mod raw_transaction;

#[derive(Clone, Deserialize)]
pub struct PredictedTransactionTemplate {
    name: String,
    target: Target,
    actual_transactions: Vec<RawTransaction>,
}

#[derive(Clone, Deserialize)]
struct Target {
    amount: RawAmount,
    starts_on: Period,
    #[serde(default)]
    until: Option<Period>,
}

impl PredictedTransactionTemplate {
    // TODO this entire function can probably be made part of the ChronoStack (works for both buckets and here)
    pub fn predicted_transactions<P: PeriodsConfiguration>(
        self,
        date: &NaiveDate,
        ex: &ExchangeRates,
    ) -> Result<Vec<PredictedTransaction>, String> {
        let period = P::period_for_date(date)?;
        // TODO I could build a ChronoStack from Payment directly here, and from an Action directly in buckets
        //      Just impl From(Payment or Action) for (NaiveDate, _)
        //      and change ChronoStack::new to take an Into Vec<(NaiveDate, _)>
        let payment_stack = ChronoStack::new(
            &self
                .actual_transactions
                .clone()
                .into_iter()
                .map(|p| p.0)
                .collect::<Vec<(NaiveDate, Period)>>(),
        )?;

        let (paid_for_before_period, paid_for_in_period_until_date, _) =
            payment_stack.into_split_for_period_and_date(&period, date);

        self.predicted_transaction_inner::<P>(
            ex,
            &period,
            paid_for_before_period,
            paid_for_in_period_until_date,
        )?
    }

    fn predicted_transaction_inner<P: PeriodsConfiguration>(
        self,
        ex: &ExchangeRates,
        period: &Period,
        paid_for_before_period: Vec<Period>,
        paid_for_in_period_until_date: Vec<Period>,
    ) -> Result<Result<Vec<PredictedTransaction>, String>, String> {
        let transaction_to_display = Self::decide_which_predicted_transactions_to_display::<P>(
            &self.target,
            &period,
            paid_for_before_period,
            paid_for_in_period_until_date,
        )?;

        Ok(transaction_to_display
            .into_iter()
            .map(|(payment_period, raw_amount)| {
                PredictedTransaction::new::<P>(
                    self.name.clone(),
                    payment_period,
                    ex.new_amount_from_raw_amount(&raw_amount)?,
                )
            })
            .collect())
    }

    fn decide_which_predicted_transactions_to_display<P: PeriodsConfiguration>(
        target: &Target,
        current_period: &Period,
        paid_for_before_period: Vec<Period>,
        paid_for_in_period_until_date: Vec<Period>,
    ) -> Result<Vec<(Period, RawAmount)>, String> {
        let (paid_for_before_period, paid_for_in_period_until_date): (
            HashSet<Period>,
            HashSet<Period>,
        ) = (
            paid_for_before_period.into_iter().collect(),
            paid_for_in_period_until_date.into_iter().collect(),
        );

        let last_period = if let Some(until) = &target.until {
            until.min(current_period)
        } else {
            current_period
        };

        let expected_transaction_periods = P::periods_between(&target.starts_on, last_period)?;

        let zero_yen: RawAmount = "¥0".try_into()?;

        let unpaid_periods = expected_transaction_periods.into_iter().filter(|period| {
            let paid_before_period = paid_for_before_period.contains(&period);
            let paid_in_period_until_date = paid_for_in_period_until_date.contains(&period);

            !paid_before_period && !paid_in_period_until_date
        });

        let paid_period = Iterator::chain(
            paid_for_in_period_until_date.iter().cloned(),
            paid_for_before_period
                .iter()
                .cloned()
                .filter(|period| period == current_period),
        );

        let mut display_periods = Iterator::chain(
            unpaid_periods
                .into_iter()
                .map(|period| (period, target.amount.clone())),
            paid_period.map(|period| (period, zero_yen.clone())),
        )
            .collect::<Vec<(Period, RawAmount)>>();

        display_periods
            .sort_by(|(left_period, _), (right_period, _)| left_period.cmp(right_period));
        Ok(display_periods)
    }
}

impl GroupBuilder<PredictedTransactionTemplate> for PredictedTransactionsVaultValue {
    fn build(self) -> Result<(String, Vec<PredictedTransactionTemplate>), String> {
        let predicted_transactions_templates: Vec<PredictedTransactionTemplate> =
            self.into_iter().collect();
        Ok((
            "Predicted Transactions".to_string(),
            predicted_transactions_templates,
        ))
    }
}

impl OperandBuilder for PredictedTransactionTemplate {
    fn build<P: PeriodsConfiguration>(
        self,
        today: &NaiveDate,
        exchange_rates: &ExchangeRates,
    ) -> Result<Vec<Operand>, String> {
        Ok(self
            .predicted_transactions::<P>(today, exchange_rates)?
            .into_iter()
            .map(|predicted_transaction| predicted_transaction.build_operand())
            .collect::<Vec<Operand>>())
    }
}

#[cfg(test)]
mod test {
    use crate::predicted_transaction::*;
    use crate::cli::formatting::render_group;
    use crate::period::CalendarMonthPeriodConfiguration;
    use crate::remaining_operation::core_types::group::Group;
    use chrono::Days;
    use pretty_assertions::assert_eq;
    use serde_json::{from_value, json};
    use crate::predicted_transaction::template::Target;
    use crate::predicted_transaction::template::raw_transaction::RawTransaction;
    use crate::predicted_transaction::transaction::PredictedTransaction;

    #[test]
    fn predicted_transactions() {
        predicted_transactions_inner::<CalendarMonthPeriodConfiguration>();
    }
    fn predicted_transactions_inner<P: PeriodsConfiguration>() {
        let periods_configuration = CalendarMonthPeriodConfiguration {};
        let today = NaiveDate::from_ymd_opt(2026, 6, 14).expect("valid date");
        let current_period = P::period_for_date(&today).expect("valid period");
        let last_period =
            P::period_for_date(&NaiveDate::from_ymd_opt(2026, 5, 14).expect("valid date"))
                .expect("could compute previous period");

        let ex = ExchangeRates::for_tests();
        let pred_trans_target_amount_raw = RawAmount::yen("1000");
        let pred_trans_target_amount = ex
            .new_amount_from_raw_amount(&pred_trans_target_amount_raw)
            .expect("valid amount");

        let configuration_name = "Spotify".to_string();

        let make_pred_trans = |period, amount| -> PredictedTransaction {
            PredictedTransaction::new::<P>(configuration_name.clone(), period, amount)
                .expect("valid predicted transaction")
        };

        struct TestCase {
            name: String,
            starting_period: Period,
            payments: Vec<RawTransaction>,
            expected_predicted_transitions: Vec<PredictedTransaction>,
        }

        let cases: Vec<TestCase> = vec![
            TestCase {
                name: "Starts this month - No payments".to_string(),
                starting_period: current_period.clone(),
                payments: vec![],
                expected_predicted_transitions: vec![make_pred_trans(
                    current_period.clone(),
                    pred_trans_target_amount.clone(),
                )],
            },
            TestCase {
                name: "Starts this month - One payment".to_string(),
                starting_period: current_period.clone(),
                payments: vec![RawTransaction((today, current_period.clone()))],
                expected_predicted_transitions: vec![make_pred_trans(
                    current_period.clone(),
                    ex.yen("0"),
                )],
            },
            TestCase {
                name: "Started last month - last month: not paid - current: not paid".to_string(),
                starting_period: last_period.clone(),
                payments: vec![],
                expected_predicted_transitions: vec![
                    make_pred_trans(last_period.clone(), pred_trans_target_amount.clone()),
                    make_pred_trans(current_period.clone(), pred_trans_target_amount.clone()),
                ],
            },
            TestCase {
                name: "Started last month - last month: paid today - current: not paid".to_string(),
                starting_period: last_period.clone(),
                payments: vec![RawTransaction((today - Days::new(4), last_period.clone()))],
                expected_predicted_transitions: vec![
                    make_pred_trans(last_period.clone(), ex.yen("0")),
                    make_pred_trans(current_period.clone(), pred_trans_target_amount.clone()),
                ],
            },
            TestCase {
                name: "Started last month - last month: paid last month - current: not paid"
                    .to_string(),
                starting_period: last_period.clone(),
                payments: vec![RawTransaction((
                    last_period.start_date + Days::new(4),
                    last_period.clone(),
                ))],
                expected_predicted_transitions: vec![make_pred_trans(
                    current_period.clone(),
                    pred_trans_target_amount.clone(),
                )],
            },
        ];

        for case in cases {
            let configuration = PredictedTransactionTemplate {
                name: configuration_name.clone(),
                target: Target {
                    amount: pred_trans_target_amount_raw.clone(),
                    starts_on: case.starting_period.clone(),
                    until: None,
                },
                actual_transactions: case.payments.clone(),
            };

            let predicted_transactions = configuration
                .clone()
                .predicted_transactions::<P>(&today, &ex)
                .expect("Predicted transaction succeeded");

            assert_eq!(
                predicted_transactions, case.expected_predicted_transitions,
                "{}",
                case.name
            )
        }
    }

    #[test]
    fn integration_test() {
        let exchange_rates = ExchangeRates::for_tests();
        let today = NaiveDate::from_ymd_opt(2026, 6, 14).expect("valid date");

        let configuration: PredictedTransactionsVaultValue = from_value(json!([
          {
            "name": "Spotify",
            "target": {
              "starts_on": "2026-04",
              "amount": "¥-2000"
            },
            "actual_transactions": [
              "2026/04/21 2026-04",
              "2026/05/21 2026-05"
            ]
          },
          {
            "name": "Electricity",
            "target": {
              "starts_on": "2026-04",
              "amount": "¥-8000"
            },
            "actual_transactions": [
              "2026/04/30 2026-04"
            ]
          },
          {
            "name": "Archived",
            "target": {
              "starts_on": "2026-04",
              "amount": "¥-8000",
              "until": "2026-05"
            },
            "actual_transactions": [
              "2026/04/30 2026-04",
              "2026/04/30 2026-05"
          ],
          }
        ]))
        .expect("valid PeriodTransactionsVaultValue");

        let group = Group::from_group_builder::<CalendarMonthPeriodConfiguration, _, _>(
            configuration,
            &exchange_rates,
            &today,
        )
        .expect("valid group");
        let group_view = group
            .into_remaining_operation_screen_group(&exchange_rates, &"JPY".to_string(), &today)
            .expect("valid group data");
        assert_eq!(
            render_group(&group_view),
            "\
Predicted Transactions
======================
+-----------------------+---------+
| Name                  | Amount  |
+=================================+
| Spotify - 2026-06     | ¥-2000  |
|-----------------------+---------|
| Electricity - 2026-05 | ¥-8000  |
|-----------------------+---------|
| Electricity - 2026-06 | ¥-8000  |
|-----------------------+---------|
| Total                 | ¥-18000 |
+-----------------------+---------+"
        );
    }

    #[test]
    fn test_monthly_payment() {
        test_monthly_payment_inner::<CalendarMonthPeriodConfiguration>();
    }
    fn test_monthly_payment_inner<P: PeriodsConfiguration>() {
        let make_period_for_month = |month: u32| {
            P::period_for_date(&NaiveDate::from_ymd_opt(2026, month, 1).expect("Can build date"))
                .expect("Can build period")
        };

        let january = make_period_for_month(1);
        let february = make_period_for_month(2);
        let march = make_period_for_month(3);
        let april = make_period_for_month(4);
        let may = make_period_for_month(5);

        struct TestCase<'a> {
            name: &'a str,

            paid_for_before_period: Vec<&'a Period>,
            paid_for_in_period_until_date: Vec<&'a Period>,
            until: Option<&'a Period>,

            expected_predicted_transitions: Vec<(&'a Period, RawAmount)>,
        };

        let cases = vec![
            TestCase {
                name: "All period paid",
                paid_for_before_period: vec![&january, &february, &march],
                paid_for_in_period_until_date: vec![&april],
                until: None,
                expected_predicted_transitions: vec![(&april, RawAmount::yen("0"))],
            },
            TestCase {
                name: "Current period not paid",
                paid_for_before_period: vec![&january, &february, &march],
                paid_for_in_period_until_date: vec![],
                until: None,
                expected_predicted_transitions: vec![(&april, RawAmount::yen("1000"))],
            },
            TestCase {
                name: "Current and last period not paid",
                paid_for_before_period: vec![&january, &february],
                paid_for_in_period_until_date: vec![],
                until: None,
                expected_predicted_transitions: vec![
                    (&march, RawAmount::yen("1000")),
                    (&april, RawAmount::yen("1000")),
                ],
            },
            TestCase {
                name: "Current and last period paid in current period",
                paid_for_before_period: vec![&january, &february],
                paid_for_in_period_until_date: vec![&march, &april],
                until: None,
                expected_predicted_transitions: vec![
                    (&march, RawAmount::yen("0")),
                    (&april, RawAmount::yen("0")),
                ],
            },
            TestCase {
                name: "No period paid",
                paid_for_before_period: vec![],
                paid_for_in_period_until_date: vec![],
                until: None,
                expected_predicted_transitions: vec![
                    (&january, RawAmount::yen("1000")),
                    (&february, RawAmount::yen("1000")),
                    (&march, RawAmount::yen("1000")),
                    (&april, RawAmount::yen("1000")),
                ],
            },
            TestCase {
                name: "paid next period in advance",
                paid_for_before_period: vec![&january, &february, &march],
                paid_for_in_period_until_date: vec![&april, &may],
                until: None,
                expected_predicted_transitions: vec![
                    (&april, RawAmount::yen("0")),
                    (&may, RawAmount::yen("0")),
                ],
            },
            TestCase {
                name: "current period paid before period",
                paid_for_before_period: vec![&january, &february, &march, &april],
                paid_for_in_period_until_date: vec![],
                until: None,
                expected_predicted_transitions: vec![(&april, RawAmount::yen("0"))],
            },
            TestCase {
                name: "archived",
                paid_for_before_period: vec![&january, &february, &march],
                paid_for_in_period_until_date: vec![],
                until: Some(&march),
                expected_predicted_transitions: vec![],
            },
            TestCase {
                name: "archived next period",
                paid_for_before_period: vec![&january, &february, &march],
                paid_for_in_period_until_date: vec![],
                until: Some(&may),
                expected_predicted_transitions: vec![(&april, RawAmount::yen("1000"))],
            },
        ];

        for case in cases {
            let target = Target {
                amount: RawAmount::yen("1000"),
                starts_on: january.clone(),
                until: case.until.cloned(),
            };
            let result =
                PredictedTransactionTemplate::decide_which_predicted_transactions_to_display::<P>(
                    &target,
                    &april,
                    case.paid_for_before_period.into_iter().cloned().collect(),
                    case.paid_for_in_period_until_date
                        .into_iter()
                        .cloned()
                        .collect(),
                )
                .expect("valid decision");

            assert_eq!(
                result,
                case.expected_predicted_transitions
                    .into_iter()
                    .map(|(period, raw_amount)| (period.clone(), raw_amount))
                    .collect::<Vec<(Period, RawAmount)>>(),
                "{}",
                case.name
            );
        }
    }
}