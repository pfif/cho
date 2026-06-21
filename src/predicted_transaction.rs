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
// TODO this file is a mess, too many structs are declared. Organize it better
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
impl GroupBuilder<PredictedTransactionTemplate> for PeriodTransactionsVaultValue {
    fn build(self) -> Result<(String, Vec<PredictedTransactionTemplate>), String> {
        let predicted_transactions_templates: Vec<PredictedTransactionTemplate> = self.into_iter().collect();
        Ok(("Predicted Transactions".to_string(),
         predicted_transactions_templates))
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
                name: format!("{} - {}", self.name, period_config.id_for_period(&period)?),
                target_amount: ex.new_amount_from_raw_amount(&self.target.amount)?,
                amount:  if has_payment {
                    // TODO Technically, I could infer the currency from the target amount, but the
                    //      Amount make up makes this hard currently. To fix once I will have fixed
                    //      amounts
                    ex.zero(&"JPY".to_string())?
                } else {
                    ex.new_amount_from_raw_amount(&self.target.amount)?
                },
                archive_from: self.archive.map(|archive_information| archive_information.on),
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
    target_amount: Amount,
    name: String,
    amount: Amount,
    archive_from: Option<NaiveDate>,
}

impl OperandBuilder for PredictedTransactionTemplate {
    fn build<P: PeriodsConfiguration>(
        self,
        period_configuration: &P,
        today: &NaiveDate,
        exchange_rates: &ExchangeRates,
    ) -> Result<Vec<Operand>, String> {
        Ok(self.
            predicted_transactions(period_configuration, today, exchange_rates)?
            .into_iter()
            .map(|predicted_transaction| predicted_transaction.build_operand())
            .collect::<Vec<Operand>>())
    }
}

impl PredictedTransaction {
    fn build_operand(self) -> Operand {
        Operand{
            name: self.name,
            amount: self.amount,
            illustration: vec![],
            archived_from: self.archive_from,
        }
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
                    // TODO arf... this is quite ugly...
                    //      this makes storing an Rc<> to the PeriodsConfiguration looks fine
                    periods_configuration.id_for_period(
                        &periods_configuration.period_for_date(&today).expect("valid_period_for_today")
                    ).expect("valid period id")),
                )],
                expected_amount: ex.yen("0")
            }
        ];

        for case in cases {
            let pred_trans_starting_period = periods_configuration.period_for_date(&today).expect("valid period");
            let pred_trans_starting_period_id = periods_configuration.id_for_period(&pred_trans_starting_period).expect("valid period id");
            let configuration_name = "Spotify".to_string();

            let archive = None;

            let configuration = PredictedTransactionTemplate {
                name: configuration_name.clone(),
                archive: archive.clone(),
                target: Target {
                    amount: RawAmount::yen("1000"),
                    starts_on: pred_trans_starting_period.clone()
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
                           name: format!("{configuration_name} - {pred_trans_starting_period_id}"),
                           target_amount: ex.new_amount_from_raw_amount(&configuration.target.amount).expect("target's amount is valid"),
                           amount: case.expected_amount,
                           archive_from: archive.map(|archive_information| archive_information.on),
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
| Electrictiy - 2026-05   | ¥8000  |
|-------------------------+--------+
| Electricity - 2026-06   | ¥8000  |
|-------------------------+--------+
| Total                   | ¥18000 |
+-------------------------+--------+");
    }

    #[test] fn todo_test_for_a_payment_outside_of_the_remaining_period(){ todo!() }
    #[test] fn todo_test_for_late_payments() { todo!() }
    #[test] fn todo_test_for_payments_much_later_than_starts_on() { todo!("display them below the table") }
    #[test] fn todo_test_for_a_cancelled_payment() { todo!("Maybe... but then again maybe there is no need for this") }
    #[test] fn todo_test_for_build_operand_forwarding_the_archive_from(){ todo!() }
    #[test] fn todo_add_a_paid_on_illustration() { todo!("The paid on illustration should show the date of the payment")}
    #[test] fn todo_keep_showing_payments_which_were_late_but_have_been_made_in_the_current_period() { todo!("Did not pay electricity last month, paid it this month for both last and current period. It should still show up")}
    #[test] fn todo_weekly_payments() { todo!("want to keep 5000 yen for every tuesday of the period")}
}