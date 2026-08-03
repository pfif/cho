/*
type PredictedTransactionVaultValue = Vec<PredictedTransactionTemplate>
impl VaultReadable for PredictedTransactionVaultValue (KEY: "predicted_transaction")
impl GroupBuilder for PredictedTransactionVaultValue
- build(): for each PredictedTransactionTemplate in Vec, call `get_predicted_transaction()` to get PredictedTransaction vecs. Flatten these vecs, and return

struct PredictedTransactionTemplate<P: PeriodConfiguration, T: Target<P>>
- display_name: String
#Serde[Default]
- archive: Option<ArchiveInformation>

- target: T
- payments: Vec<Payment>
- get_predicted_transaction(this_period): Vec<PredictedTransaction>
    - payment_period = loop over self.target.when.periods_between(period_start: self.target.starts_on, period_end: this_period.end)
        - If payment_period.id in [p.period_id for p in self.payments]
            - Construct PredictedTransaction with payment
        - Else
            - Construct PredictedTransaction without payment

struct Target<P: PeriodsConfiguration>
- expected_amount: RawAmount
- when: P
- starts_on: NaiveDate

#[derive(Deserialize)]
struct ArchiveInformation
- on: NaiveDate
  // Loose string to record information about the archival - for instance, what PredictedTransactionTemplate this replaces
- comment: Option<String>

#[derive(Deserialize)]
struct Payment
    on: NaiveDate
    period_id: String
    // Unused for now, but kept as this may be interesting data
    amount: RawAmount

struct PredictedTransaction
- name: "Predicted transaction - {period, rendered through Display}"
- expected_amount: RawAmount
- period: Period
- payment: Option<Payment>
impl OperandBuilder for PredictedTransaction:
- build():
  - Operand.name is self.name
  - Operand.amount is (self.expected_amount if self.payment is None else (0 if self.payment_date >= today else self.expected_amount) )
  - Operand.illustration is [
        ("period_id", self.period.id)
)]
 */
use std::collections::HashSet;
// TODO this file is a mess, too many structs are declared. Organize it better
use crate::amounts::exchange_rates::ExchangeRates;
use crate::amounts::{Amount, RawAmount};
use crate::chrono_stack::ChronoStack;
use crate::line::LineWithDateVisitor;
use crate::period::{Period, PeriodsConfiguration};
use crate::remaining_operation::core_types::{GroupBuilder, Operand, OperandBuilder};
use crate::vault::VaultReadable;
use chrono::NaiveDate;
use serde::de::{Error, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt::{Debug, Formatter};

pub type PeriodTransactionsVaultValue = Vec<PredictedTransactionTemplate>;
impl VaultReadable for PeriodTransactionsVaultValue {
    const KEY: &'static str = "predicted_transactions";
}
impl GroupBuilder<PredictedTransactionTemplate> for PeriodTransactionsVaultValue {
    fn build(self) -> Result<(String, Vec<PredictedTransactionTemplate>), String> {
        let predicted_transactions_templates: Vec<PredictedTransactionTemplate> =
            self.into_iter().collect();
        Ok((
            "Predicted Transactions".to_string(),
            predicted_transactions_templates,
        ))
    }
}

#[derive(Clone, Deserialize)]
struct PredictedTransactionTemplate {
    name: String,
    #[serde(default)]
    archive: Option<ArchiveInformation>,
    target: Target,
    payments: Vec<Payment>,
}

impl PredictedTransactionTemplate {
    // TODO this entire function can probably be made part of the ChronoStack (works for both buckets and here)
    pub fn predicted_transactions<P: PeriodsConfiguration>(
        self,
        period_config: &P,
        date: &NaiveDate,
        ex: &ExchangeRates,
    ) -> Result<Vec<PredictedTransaction>, String> {
        let period = period_config.period_for_date(date)?;
        // TODO I could build a ChronoStack from Payment directly here, and from an Action directly in buckets
        //      Just impl From(Payment or Action) for (NaiveDate, _)
        //      and change ChronoStack::new to take an Into Vec<(NaiveDate, _)>
        let payment_stack = ChronoStack::new(
            &self
                .payments
                .clone()
                .into_iter()
                .map(|p| p.0)
                .collect::<Vec<(NaiveDate, Period)>>(),
        )?;

        let (paid_for_before_period, paid_for_in_period_until_date, _) =
            payment_stack.into_split_for_period_and_date(&period, date);

        self.predicted_transaction_inner(
            period_config,
            ex,
            &period,
            paid_for_before_period,
            paid_for_in_period_until_date,
        )?
    }

    fn predicted_transaction_inner<P: PeriodsConfiguration>(
        self,
        period_config: &P,
        ex: &ExchangeRates,
        period: &Period,
        paid_for_before_period: Vec<Period>,
        paid_for_in_period_until_date: Vec<Period>,
    ) -> Result<Result<Vec<PredictedTransaction>, String>, String> {
        let transaction_to_display = Self::decide_which_predicted_transactions_to_display(
            period_config,
            &self.target,
            &period,
            paid_for_before_period,
            paid_for_in_period_until_date,
        )?;

        Ok(transaction_to_display
            .into_iter()
            .map(|(payment_period, raw_amount)| {
                PredictedTransaction::new(
                    period_config,
                    self.name.clone(),
                    payment_period,
                    ex.new_amount_from_raw_amount(&raw_amount)?,
                    self.archive
                        .clone()
                        .map(|archive_information| archive_information.on),
                )
            })
            .collect())
    }

    fn decide_which_predicted_transactions_to_display<P: PeriodsConfiguration>(
        period_config: &P,
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

        let expected_transaction_periods =
            period_config.periods_between(&target.starts_on, &current_period)?;

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

#[derive(Clone, Deserialize)]
struct ArchiveInformation {
    on: NaiveDate,
    /// Lose string to record information about the archival -
    /// for instance, what PredictedTransactionTemplate replaces this one, if any
    comment: Option<String>,
}

/// A payment is a date and a period id.
#[derive(Clone, Debug)]
struct Payment((NaiveDate, Period));
// TODO hash impl that only takes period into account [gosh that feels wrooooong]

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

// As of now, the Target assumes that the Period is valid for for the RemainingOperation's
// PeriodsConfiguration, but I keep thinking of usage for other kind of targets
// - Putting money on the side for a date later in the month
// - Keeping money for my weekly Tuesday downtown hang, or weekly activities (which is an instance of the above, but regular)
// Therefore, we will have several kind of Target. Good to keep in mind
#[derive(Clone, Deserialize)]
struct Target {
    amount: RawAmount,
    starts_on: Period,
}

#[derive(Debug, PartialEq, Eq)]
struct PredictedTransaction {
    name: String,
    amount: Amount,
    archive_from: Option<NaiveDate>,
}

impl PredictedTransaction {
    // TODO One more reason to make period hold their own configuration? If so, we woun't need to
    //      pass in a PeriodConfig here.
    //      On the one hand, it's nice to make clear where the PeriodConfig is needed
    //      On the other hand it's verbose.
    //      Maybe the inbetween solution is to not make the PeriodConfiguration required to compute the ID. Idk
    //
    //      Wait. I could just ... encode the id for the period in the period at construction time ...
    //      So simple
    fn new<P: PeriodsConfiguration>(
        period_config: &P,
        template_name: String,
        period: Period,
        amount: Amount,
        archive_from: Option<NaiveDate>,
    ) -> Result<PredictedTransaction, String> {
        Ok(PredictedTransaction {
            name: format!(
                "{} - {}",
                template_name,
                period_config.id_for_period(&period)?
            ),
            amount,
            archive_from,
        })
    }
}

impl OperandBuilder for PredictedTransactionTemplate {
    fn build<P: PeriodsConfiguration>(
        self,
        period_configuration: &P,
        today: &NaiveDate,
        exchange_rates: &ExchangeRates,
    ) -> Result<Vec<Operand>, String> {
        Ok(self
            .predicted_transactions(period_configuration, today, exchange_rates)?
            .into_iter()
            .map(|predicted_transaction| predicted_transaction.build_operand())
            .collect::<Vec<Operand>>())
    }
}

impl PredictedTransaction {
    fn build_operand(self) -> Operand {
        Operand {
            name: self.name,
            amount: self.amount,
            illustration: vec![],
            archived_from: self.archive_from,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::cli::formatting::render_group;
    use crate::period::CalendarMonthPeriodConfiguration;
    use crate::remaining_operation::core_types::group::Group;
    use chrono::Days;
    use pretty_assertions::assert_eq;
    use serde_json::{from_value, json};

    mod predicted_transaction_template {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn predicted_transactions() {
            let periods_configuration = CalendarMonthPeriodConfiguration {};
            let today = NaiveDate::from_ymd_opt(2026, 6, 14).expect("valid date");
            let current_period = periods_configuration
                .period_for_date(&today)
                .expect("valid period");
            let last_period = periods_configuration
                .period_for_date(&NaiveDate::from_ymd_opt(2026, 5, 14).expect("valid date"))
                .expect("could compute previous period");

            let ex = ExchangeRates::for_tests();
            let pred_trans_target_amount_raw = RawAmount::yen("1000");
            let pred_trans_target_amount = ex
                .new_amount_from_raw_amount(&pred_trans_target_amount_raw)
                .expect("valid amount");

            let configuration_name = "Spotify".to_string();

            let archive: Option<ArchiveInformation> = None;
            let make_pred_trans = |period, amount| -> PredictedTransaction {
                PredictedTransaction::new(
                    &periods_configuration,
                    configuration_name.clone(),
                    period,
                    amount,
                    None, // Should be a reference to archive above, but it cannot be moved into the
                          // closure because it does not implement Copy
                          // TODO solve that?
                )
                .expect("valid predicted transaction")
            };

            struct TestCase {
                name: String,
                starting_period: Period,
                payments: Vec<Payment>,
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
                    payments: vec![Payment((today, current_period.clone()))],
                    expected_predicted_transitions: vec![make_pred_trans(
                        current_period.clone(),
                        ex.yen("0"),
                    )],
                },
                TestCase {
                    name: "Started last month - last month: not paid - current: not paid"
                        .to_string(),
                    starting_period: last_period.clone(),
                    payments: vec![],
                    expected_predicted_transitions: vec![
                        make_pred_trans(last_period.clone(), pred_trans_target_amount.clone()),
                        make_pred_trans(current_period.clone(), pred_trans_target_amount.clone()),
                    ],
                },
                TestCase {
                    name: "Started last month - last month: paid today - current: not paid"
                        .to_string(),
                    starting_period: last_period.clone(),
                    payments: vec![Payment((today - Days::new(4), last_period.clone()))],
                    expected_predicted_transitions: vec![
                        make_pred_trans(last_period.clone(), ex.yen("0")),
                        make_pred_trans(current_period.clone(), pred_trans_target_amount.clone()),
                    ],
                },
                TestCase {
                    name: "Started last month - last month: paid last month - current: not paid"
                        .to_string(),
                    starting_period: last_period.clone(),
                    payments: vec![Payment((
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
                    archive: archive.clone(),
                    target: Target {
                        amount: pred_trans_target_amount_raw.clone(),
                        starts_on: case.starting_period.clone(),
                    },
                    payments: case.payments.clone(),
                };

                let predicted_transactions = configuration
                    .clone()
                    .predicted_transactions(&periods_configuration, &today, &ex)
                    .expect("Predicted transaction succeeded");

                assert_eq!(
                    predicted_transactions, case.expected_predicted_transitions,
                    "{}",
                    case.name
                )
            }
        }
    }

    #[test]
    fn integration_test() {
        let exchange_rates = ExchangeRates::for_tests();
        let periods_configuration = CalendarMonthPeriodConfiguration {};
        let today = NaiveDate::from_ymd_opt(2026, 6, 14).expect("valid date");

        let configuration: PeriodTransactionsVaultValue = from_value(json!([
          {
            "name": "Spotify",
            "target": {
              "starts_on": "2026-04",
              "amount": "¥2000"
            },
            "payments": [
              "2026/04/21 2026-04",
              "2026/05/21 2026-05"
            ]
          },
          {
            "name": "Electricity",
            "target": {
              "starts_on": "2026-04",
              "amount": "¥8000"
            },
            "payments": [
              "2026/04/30 2026-04"
            ]
          }
        ]))
        .expect("valid PeriodTransactionsVaultValue");

        let group = Group::from_group_builder(
            configuration,
            &exchange_rates,
            &periods_configuration,
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
+-----------------------+--------+
| Name                  | Amount |
+================================+
| Spotify - 2026-06     | ¥2000  |
|-----------------------+--------|
| Electricity - 2026-05 | ¥8000  |
|-----------------------+--------|
| Electricity - 2026-06 | ¥8000  |
|-----------------------+--------|
| Total                 | ¥18000 |
+-----------------------+--------+"
        );
    }

    #[test]
    fn test_monthly_payment(
    ) {
        let period_config = CalendarMonthPeriodConfiguration {};
        let make_period_for_month = |month: u32| {
            period_config
                .period_for_date(&NaiveDate::from_ymd_opt(2026, month, 1).expect("Can build date"))
                .expect("Can build period")
        };

        let january = make_period_for_month(1);
        let february = make_period_for_month(2);
        let march = make_period_for_month(3);
        let april = make_period_for_month(4);
        let may = make_period_for_month(5);

        let target = Target {
            amount: RawAmount::yen("1000"),
            starts_on: january.clone(),
        };

        struct TestCase<'a> {
            name: &'a str,

            paid_for_before_period: Vec<&'a Period>,
            paid_for_in_period_until_date: Vec<&'a Period>,

            expected_predicted_transitions: Vec<(&'a Period, RawAmount)>,
        };

        let cases = vec![
            TestCase {
                name: "All period paid",
                paid_for_before_period: vec![&january, &february, &march],
                paid_for_in_period_until_date: vec![&april],
                expected_predicted_transitions: vec![(&april, RawAmount::yen("0"))],
            },
            TestCase {
                name: "Current period not paid",
                paid_for_before_period: vec![&january, &february, &march],
                paid_for_in_period_until_date: vec![],
                expected_predicted_transitions: vec![(&april, RawAmount::yen("1000"))],
            },
            TestCase {
                name: "Current and last period not paid",
                paid_for_before_period: vec![&january, &february],
                paid_for_in_period_until_date: vec![],
                expected_predicted_transitions: vec![
                    (&march, RawAmount::yen("1000")),
                    (&april, RawAmount::yen("1000")),
                ],
            },
            TestCase {
                name: "Current and last period paid in current period",
                paid_for_before_period: vec![&january, &february],
                paid_for_in_period_until_date: vec![&march, &april],
                expected_predicted_transitions: vec![
                    (&march, RawAmount::yen("0")),
                    (&april, RawAmount::yen("0")),
                ],
            },
            TestCase {
                name: "No period paid",
                paid_for_before_period: vec![],
                paid_for_in_period_until_date: vec![],
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
                expected_predicted_transitions: vec![
                    (&april, RawAmount::yen("0")),
                    (&may, RawAmount::yen("0")),
                ],
            },
            TestCase {
                name: "current period paid before period",
                paid_for_before_period: vec![&january, &february, &march, &april],
                paid_for_in_period_until_date: vec![],
                expected_predicted_transitions: vec![(&april, RawAmount::yen("0"))],
            },
        ];

        for case in cases {
            let result =
                PredictedTransactionTemplate::decide_which_predicted_transactions_to_display(
                    &period_config,
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
    #[test]
    fn todo_test_for_payments_much_later_than_starts_on() {
        todo!("display them below the table")
    }
    #[test]
    fn todo_test_for_a_cancelled_payment() {
        todo!("Maybe... but then again maybe there is no need for this")
    }
    #[test]
    fn todo_test_for_build_operand_forwarding_the_archive_from() {
        todo!()
    }
    #[test]
    fn todo_keep_showing_payments_which_were_late_but_have_been_made_in_the_current_period() {
        todo!("Did not pay electricity last month, paid it this month for both last and current period. It should still show up")
    }
    #[test]
    fn todo_weekly_payments() {
        todo!("want to keep 5000 yen for every tuesday of the period")
    }
    #[test]
    fn todo_predicted_transaction_new() {
        todo!("Currently used by the 'predicted_transaction_template.predicted_transactions' test, but inner not tested")
    }

    #[test]
    fn todo_payments_out_of_order() {
        todo!("This is optional as it's already tested back and forth in chrono_stack.rs")
    }

    #[test]
    fn todo_boundary_check() {
        todo!("payment today, for current period, payment tomorrow, for next period. そんな感じ")
    }
}
