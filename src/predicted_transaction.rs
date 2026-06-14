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
use std::fmt::{Debug, Formatter};
use chrono::NaiveDate;
use serde::{Deserialize, Deserializer};
use serde::de::{Error, Visitor};
use crate::amounts::exchange_rates::ExchangeRates;
use crate::amounts::{Amount, RawAmount};
use crate::line::LineWithDateVisitor;
use crate::period::{Period, PeriodsConfiguration};
use crate::remaining_operation::core_types::{GroupBuilder, Operand, OperandBuilder};
use crate::vault::VaultReadable;

pub type PeriodTransactionsVaultValue = Vec<PredictedTransactionTemplate>;
impl VaultReadable for PeriodTransactionsVaultValue {
    const KEY: &'static str = "predicted_transactions";
}
impl GroupBuilder<PredictedTransaction> for PeriodTransactionsVaultValue {
    fn build(self) -> Result<(String, Vec<PredictedTransaction>), String> {
        todo!()
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
    pub fn predicted_transactions<P: PeriodsConfiguration>(
        self,
        period_config: &P,
        date: &NaiveDate,
        ex: &ExchangeRates,
    ) -> Result<Vec<PredictedTransaction>, String> {
        let period = period_config.period_for_date(date)?;
        let has_payment = self.payments.into_iter().next().is_some();

        Ok(vec![
            PredictedTransaction {
                name: format!("{} - {}", self.name, period),
                target_amount: ex.new_amount_from_raw_amount(&self.target.amount)?,
                period_id: period.id(),
                amount:  if has_payment {
                    ex.zero(&"JPY".to_string())?
                } else {
                    ex.new_amount_from_raw_amount(&self.target.amount)?
                }
            },
        ])
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
#[derive(Clone)]
struct Payment((NaiveDate, String));

impl<'de> Deserialize<'de> for Payment {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>
    {
        struct PeriodIDVisitor;
        impl<'de> Visitor<'de> for PeriodIDVisitor {
            type Value = String;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter.write_str("a period id")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: Error
            {
                Ok(v.to_string())
            }
        }
        let visitor = LineWithDateVisitor::new(PeriodIDVisitor);
        let tuple = deserializer.deserialize_str(visitor)?;
        Ok(Payment(tuple))
    }
}

#[derive(Clone, Deserialize)]
struct Target {
    amount: RawAmount,
    starts_on: NaiveDate,
}

#[derive(Debug, PartialEq, Eq)]
struct PredictedTransaction {
    target_amount: Amount,
    period_id: String,
    name: String,
    amount: Amount,
}

impl OperandBuilder for PredictedTransaction {
    fn build<P: PeriodsConfiguration>(
        self,
        period_configuration: &P,
        today: &NaiveDate,
        exchange_rates: &ExchangeRates,
    ) -> Result<Option<Operand>, String> {
        todo!()
    }
}

#[cfg(test)]
mod test {
    use chrono::Days;
    use super::*;
    use crate::period::CalendarMonthPeriodConfiguration;
    use pretty_assertions::assert_eq;
    use serde_json::{from_value, json};
    use crate::cli::formatting::render_group;
    use crate::remaining_operation::core_types::group::Group;

    #[test]
    fn test(){
        let periods_configuration = CalendarMonthPeriodConfiguration {};
        let pred_trans_target_amount = RawAmount::yen("1000");
        let today = NaiveDate::from_ymd_opt(2026, 6, 14).expect("valid date");
        let ex = ExchangeRates::for_tests();

        struct TestCase {
            name: String,
            payments: Vec<Payment>,
            expected_amount: Amount,
        }

        let cases: Vec<TestCase> = vec![
            TestCase{
                name: "No payments".to_string(),
                payments: vec![],
                expected_amount: ex.new_amount_from_raw_amount(&pred_trans_target_amount).expect("valid amount")
            },
            TestCase {
                name: "One payment".to_string(),
                payments: vec![Payment((
                    today - Days::new(4),
                    periods_configuration.period_for_date(&today).expect("valid_period_for_today").id(),
                ))],
                expected_amount: ex.yen("0")
            }
        ];

        for case in cases {
            let pred_trans_started_on = NaiveDate::from_ymd_opt(2025, 5, 7).expect("valid_date");
            let configuration_name = "Spotify".to_string();

            let month_start = NaiveDate::from_ymd_opt(2026, 6, 1).expect("valid date");
            let month_end = NaiveDate::from_ymd_opt(2026, 6, 30).expect("valid date");

            let configuration = PredictedTransactionTemplate {
                name: configuration_name.clone(),
                archive: None,
                target: Target {
                    amount: RawAmount::yen("1000"),
                    starts_on: pred_trans_started_on.clone()
                },
                payments: case.payments,
            };



            let predicted_transactions = configuration.clone().predicted_transactions(
                &periods_configuration,
                &today,
                &ex,
            ).expect("Predicted transaction succeeded");

            let [predicted_transaction]: [_; 1] = predicted_transactions.try_into().expect("only one predicted transaction");

            assert_eq!(predicted_transaction,
                       PredictedTransaction {
                           name: format!("{configuration_name} - {month_start} to {month_end}"),
                           target_amount: ex.new_amount_from_raw_amount(&configuration.target.amount).expect("target's amount is valid"),
                           amount: case.expected_amount,
                           period_id: periods_configuration.period_for_date(&today).expect("can create period for today").id(),
                       },
                       "{}",
                       case.name
            );
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
])).expect("valid PeriodTransactionsVaultValue");

        let group = Group::from_group_builder(configuration, &exchange_rates, &periods_configuration, &today).expect("valid group");
        let group_view = group.into_remaining_operation_screen_group(
            &exchange_rates, &"JPY".to_string(), &today).expect("valid group data");
        assert_eq!(render_group(&group_view ),
                   "\
Normal group
============
+-------------------------+--------+
| Name                    | Amount |
+===================================
| Spotify - 2026-06       | ¥2000  |
|-------------------------+--------+
| Eletrictiy - 2026-05    | ¥8000  |
|-------------------------+--------+
| Electricity - 2026-06   | ¥8000  |
|-------------------------+--------+
| Total                   | ¥18000 |
+-------------------------+--------+");
    }

    #[test] fn todo_test_for_a_payment_outside_of_the_remaining_period(){ todo!() }
    #[test] fn todo_test_for_late_payments() { todo!() }
    #[test] fn todo_test_for_payments_much_later_than_starts_on() { todo!() }
    #[test] fn todo_test_for_a_cancelled_payment() { todo!("Maybe... but then again maybe there is no need for this") }
}