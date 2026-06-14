use std::ops::Add;
use crate::amounts::exchange_rates::ExchangeRates;
use crate::amounts::{Amount, CurrencyIdent};
use crate::period::{Period, PeriodConfigurationVaultValue, PeriodsConfiguration};
use chrono::{Local, NaiveDate};
use group::Group;
use rust_decimal_macros::dec;
use crate::accounts::AccountGetter;
use crate::buckets::BucketsVaultValue;
use crate::ignored_transaction::IgnoredTransactionsVaultValues;
use crate::predicted_income::PredictedIncome;
use crate::vault::{Vault, VaultReadable};

/* Entrypoint */
pub struct RemainingOperation {
    groups: Vec<Group>,
    periods_configuration: PeriodConfigurationVaultValue,
    date: NaiveDate,
    exchange_rates: ExchangeRates,
}

impl RemainingOperation {
    pub fn new(
        periods_configuration: PeriodConfigurationVaultValue,
        date: NaiveDate,
        exchange_rates: ExchangeRates,
    ) -> RemainingOperation {
        RemainingOperation {
            groups: Vec::new(),
            periods_configuration,
            date,
            exchange_rates,
        }
    }
    pub fn from_vault_values<V: Vault>(
        include_predicted_income: bool,
        vault: &V,
        exchange_rates: ExchangeRates,
    ) -> Result<RemainingOperation, String> {
        let mut operation = RemainingOperation::new(
            PeriodConfigurationVaultValue::from_vault(vault)?,
            Local::now().date_naive(),
            exchange_rates,
        );
        operation.add_group(AccountGetter::from_vault(vault)?)?;
        operation.add_group(BucketsVaultValue::from_vault(vault)?)?;
        operation.add_group(IgnoredTransactionsVaultValues::from_vault(vault)?)?;
        if include_predicted_income {
            operation.add_group(PredictedIncome::from_vault(vault)?)?;
        }
        Ok(operation)
    }

    pub fn add_group<O: OperandBuilder, B: GroupBuilder<O>>(&mut self, builder: B) -> Result<(), String> {
        let group = Group::from_group_builder(builder, &self.exchange_rates, &self.periods_configuration, &self.date)?;
        self.groups.push(group);
        Ok(())
    }

    pub fn execute(
        &self,
        target_currency: &CurrencyIdent,
    ) -> Result<RemainingOperationScreen, String> {

        let current_period = self
            .periods_configuration
            .period_for_date(&self.date)
            .map_err(|error| "Failed to fetch Periods Configuration: ".to_string() + &error)?;

        let remaining_operation_screen_group = self.groups
            .clone()
            .into_iter()
            .map(|group| group.into_remaining_operation_screen_group(
                &self.exchange_rates, target_currency, &self.date))
            .collect::<Result<Vec<RemainingOperationScreenGroup>, String>>()?;

        let remaining = remaining_operation_screen_group.iter().fold(
            self.exchange_rates.new_amount(target_currency, dec!(0))?,
            |total, subtotal| total + subtotal.total.clone());

        Ok(RemainingOperationScreen {
            period: current_period,
            groups: remaining_operation_screen_group,
            remaining,
        })
    }
}

/* Builders */
pub trait GroupBuilder<B: OperandBuilder> {
    fn build(
        self,
    ) -> Result<(String, Vec<B>), String>;
}

pub trait OperandBuilder {
    fn build<P: PeriodsConfiguration>(
        self,
        period_configuration: &P,
        today: &NaiveDate,
        // Exchange rate is only necessary because other parts of the codebase need to convert their understanding of currency into Amounts produced by Exchange rates
        // Once the entire codebase adopts ExchangeRates, we won't need to pass it around
        // TODO Remove exchange rate from this interface
        exchange_rates: &ExchangeRates,
    ) -> Result<Option<Operand>, String>;
}

/* Output types */
#[derive(PartialEq, Debug, Eq)]
pub struct RemainingOperationScreen {
    pub groups: Vec<RemainingOperationScreenGroup>,
    pub remaining: Amount,
    pub period: Period,
}

#[derive(PartialEq, Debug, Eq)]
pub struct RemainingOperationScreenGroup {
    pub name: String,
    pub operands: Vec<RemainingOperationScreenOperand>,
    pub illustration_fields: Vec<String>,
    pub total: Amount,
    /// The name of Operands marked as archived but whose amount is not zero
    /// A warning about this should be displayed to the user, specifying that
    /// its math still affects the amount for the group and thus for the
    /// remaining operation
    pub archived_operand_with_non_zero_amounts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemainingOperationScreenOperand {
    pub name: String,
    pub amount: Amount,
    pub illustration: Illustration,
}

impl From<Operand> for RemainingOperationScreenOperand {
    fn from(original: Operand) -> Self {
        RemainingOperationScreenOperand{
            name: original.name,
            amount: original.amount,
            illustration: original.illustration
        }
    }
}

// The struct Group has its own module to isolate its internal attribute
pub mod group {
    use std::ops::Add;
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;
    use crate::period::{PeriodConfigurationVaultValue, PeriodsConfiguration};
    use crate::amounts::{Amount, Currency, CurrencyIdent};
    use crate::amounts::exchange_rates::ExchangeRates;
    use super::{GroupBuilder, Operand, OperandBuilder, RemainingOperationScreenGroup, RemainingOperationScreenOperand};

    #[derive(Clone, PartialEq, Eq, Debug)]
    pub struct Group {
        name: String,
        operands: Vec<Operand>,
        illustration_fields: Option<Vec<String>>,
    }


    impl Group {
        pub fn new(name: &str, operands: Vec<Operand>) -> Result<Group, String> {
            let mut group = Group {
                name: name.to_string(),
                operands: vec![],
                illustration_fields: None,
            };

            for operand in operands {
                group.add_operands(operand)?;
            }

            Ok(group)
        }

        pub fn from_group_builder<P: PeriodsConfiguration, O: OperandBuilder, B: GroupBuilder<O>>(
            group_builder: B,
            exchange_rates: &ExchangeRates,
            period_configuration: &P,
            today: &NaiveDate,
        ) -> Result<Group, String>{
            let (name, operand_builders) = group_builder.build()?;

            let operands: Vec<Operand> = operand_builders
                .into_iter()
                .map(|operand_builder| operand_builder.build(period_configuration, today, &exchange_rates))
                .collect::<Result<Vec<Option<Operand>>, String>>()?
                .into_iter()
                .filter_map(|operand| operand)
                .collect();

            Group::new(&name, operands)
        }

        pub fn add_operands(&mut self, o: Operand) -> Result<(), String> {
            if self.illustration_fields == None {
                let fields = o.illustration.iter().map(|(k, _)| k.clone()).collect();
                self.illustration_fields = Some(fields);
            } else if let Some(illustration_fields) = &self.illustration_fields {
                let field_names: Vec<String> = o.illustration.iter().map(|(k, _)| k.clone()).collect();
                if field_names.iter().ne(illustration_fields) {
                    return Err(format!(
                        "Adding an operand ({:?}) whose fields ({:?}) does not match that of the rest of the operand in this group ({:?})",
                        o.name,
                        field_names,
                        illustration_fields
                    ));
                }
            }
            self.operands.push(o);
            Ok(())
        }

        pub fn name(&self) -> &String { &self.name }
        pub fn operands(&self) -> &Vec<Operand> {
            &self.operands
        }

        pub fn into_remaining_operation_screen_group(
            self,
            exchange_rates: &ExchangeRates,
            target_currency: &CurrencyIdent,
            date: &NaiveDate,
        ) -> Result<RemainingOperationScreenGroup, String> {
            let total = self.operands
                .iter()
                .fold(exchange_rates.new_amount(target_currency, dec!(0))?, |acc, operand| {
                    acc + operand.amount.clone()
                });

            let mut operands: Vec<RemainingOperationScreenOperand> = vec![];
            let mut archived_operand_with_non_zero_amounts = vec![];
            self.operands.iter().for_each(
                |o | {
                    let should_include_operand = match o.archived_from {
                        | None => true,
                        | Some(operand_archive_date) if *date < operand_archive_date  => true,
                        _ => false
                    };
                    if should_include_operand {
                        operands.push(o.clone().into());
                    } else {
                        if !o.amount.is_zero() {
                            archived_operand_with_non_zero_amounts.push(o.name.clone());
                        }
                    }
                });

           Ok(RemainingOperationScreenGroup{
               name: self.name,
               operands,
               illustration_fields: self.illustration_fields.unwrap_or(vec![]),
               total,
               archived_operand_with_non_zero_amounts
           })
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IllustrationValue {
    Amount(Amount),
    Null,
    Bool(bool),
    Date(NaiveDate),
}

impl From<Amount> for IllustrationValue {
    fn from(value: Amount) -> Self {
        IllustrationValue::Amount(value)
    }
}

impl From<NaiveDate> for IllustrationValue {
    fn from(value: NaiveDate) -> Self {
        IllustrationValue::Date(value)
    }
}

impl<T: Into<IllustrationValue>> From<Option<T>> for IllustrationValue {
    fn from(value: Option<T>) -> Self {
       value
           .map(|value| value.into())
           .unwrap_or(IllustrationValue::Null)
    }
}

pub type Illustration = Vec<(String, IllustrationValue)>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operand {
    pub name: String,
    pub amount: Amount,
    pub illustration: Illustration,
    pub archived_from: Option<NaiveDate>
}

#[cfg(test)]
mod test {
    mod group_test {
        use std::ops::Add;
        use chrono::{Months, NaiveDate};
        use crate::amounts::exchange_rates::ExchangeRates;
        use crate::remaining_operation::core_types::group::Group;
        use crate::remaining_operation::core_types::{IllustrationValue, Operand, RemainingOperationScreenGroup, RemainingOperationScreenOperand};
        use pretty_assertions::assert_eq;

        #[test]
        fn one_operand() {
            let date = NaiveDate::from_ymd_opt(2026,5, 9).expect("Can create date");
            let ex = ExchangeRates::for_tests();

            let name = "foo".to_string();
            let amount = ex.yen("500").clone();
            let illustration = vec![
                ("Default amount".to_string(), IllustrationValue::Amount(ex.yen("300")))
            ];
            let operand = Operand {
                name: name.to_string(),
                amount: amount.clone(),
                illustration: illustration.clone(),
                archived_from: None,
            };

            let group_name = "bar";

            let group = Group::new(group_name, vec![operand.clone()])
                .expect("Can create group");
            let result = group
                .into_remaining_operation_screen_group(&ex, &"JPY".to_string(), &date)
                .expect("can build RemainingOperationScreenGroup");
            let expected = RemainingOperationScreenGroup {
                name: group_name.to_string(),
                total: operand.amount.clone(),
                operands: vec![RemainingOperationScreenOperand{
                    name, amount, illustration
                }],
                illustration_fields: vec!["Default amount".to_string()],
                archived_operand_with_non_zero_amounts: vec![]
            };
            assert_eq!(result, expected)
        }

        #[test]
        fn two_operands() {
            let date = NaiveDate::from_ymd_opt(2026,5, 9).expect("Can create date");
            let ex = ExchangeRates::for_tests();

            let name_left = "foo".to_string();
            let amount_left = ex.yen("500").clone();
            let illustration_left = vec![
                ("Default amount".to_string(), IllustrationValue::Amount(ex.yen("300")))
            ];
            let operand_left = Operand {
                name: name_left.clone(),
                amount: amount_left.clone(),
                illustration: illustration_left.clone(),
                archived_from: None,
            };

            let name_right = "baz".to_string();
            let amount_right = ex.yen("1000").clone();
            let illustration_right = vec![
                ("Default amount".to_string(), IllustrationValue::Amount(ex.yen("1300")))
            ];
            let operand_right = Operand {
                name: name_right.clone(),
                amount: amount_right.clone(),
                illustration: illustration_right.clone(),
                archived_from: None,
            };

            let group_name = "bar";

            let group = Group::new(group_name, vec![operand_left.clone(), operand_right.clone()])
                .expect("Can create group");
            let result = group
                .into_remaining_operation_screen_group(&ex, &"JPY".to_string(), &date)
                .expect("can build RemainingOperationScreenGroup");
            let expected = RemainingOperationScreenGroup {
                name: group_name.to_string(),
                total: operand_left.amount + operand_right.amount,
                operands: vec![
                    RemainingOperationScreenOperand{
                        name: name_left, amount: amount_left, illustration: illustration_left},
                    RemainingOperationScreenOperand{
                        name: name_right, amount: amount_right, illustration: illustration_right
                    }],
                illustration_fields: vec!["Default amount".to_string()],
                archived_operand_with_non_zero_amounts: vec![]
            };
            assert_eq!(result, expected)
        }

        #[test]
        fn operands_with_different_fields() {
            let ex = ExchangeRates::for_tests();

            let operand_left = Operand {
                name: "foo".to_string(),
                amount: ex.yen("500").clone(),
                illustration: vec![
                    ("Default amount".to_string(), IllustrationValue::Amount(ex.yen("300")))
                ],
                archived_from: None,
            };

            let operand_right = Operand {
                name: "baz".to_string(),
                amount: ex.yen("1000").clone(),
                illustration: vec![
                    ("Default amount".to_string(), IllustrationValue::Amount(ex.yen("1300"))),
                    ("Field that should not be there".to_string(), IllustrationValue::Null)
                ],
                archived_from: None,
            };

            let group_name = "bar";

            let result = Group::new(group_name, vec![operand_left.clone(), operand_right.clone()]);
            let expected = Err("Adding an operand (\"baz\") whose fields ([\"Default amount\", \"Field that should not be there\"]) does not match that of the rest of the operand in this group ([\"Default amount\"])".to_string());
            assert_eq!(result, expected);
        }

        #[test]
        fn two_operands_archived_later() {
            let date = NaiveDate::from_ymd_opt(2026,5, 9).expect("Can create date");
            let ex = ExchangeRates::for_tests();

            let name_left = "foo".to_string();
            let amount_left = ex.yen("500").clone();
            let illustration_left = vec![
                ("Default amount".to_string(), IllustrationValue::Amount(ex.yen("300")))
            ];
            let operand_left = Operand {
                name: name_left.clone(),
                amount: amount_left.clone(),
                illustration: illustration_left.clone(),
                archived_from: Some(
                    date.checked_add_months(Months::new(1)).expect("can add a month to the date")),
            };

            let name_right = "baz".to_string();
            let amount_right = ex.yen("1000").clone();
            let illustration_right = vec![
                ("Default amount".to_string(), IllustrationValue::Amount(ex.yen("1300")))
            ];
            let operand_right = Operand {
                name: name_right.clone(),
                amount: amount_right.clone(),
                illustration: illustration_right.clone(),
                archived_from: Some(
                    date.checked_add_months(Months::new(1)).expect("can add a month to the date")),
            };

            let group_name = "bar";

            let group = Group::new(group_name, vec![operand_left.clone(), operand_right.clone()])
                .expect("Can create group");
            let result = group
                .into_remaining_operation_screen_group(&ex, &"JPY".to_string(), &date)
                .expect("can build RemainingOperationScreenGroup");
            let expected = RemainingOperationScreenGroup {
                name: group_name.to_string(),
                total: operand_left.amount + operand_right.amount,
                operands: vec![
                    RemainingOperationScreenOperand{
                        name: name_left, amount: amount_left, illustration: illustration_left},
                    RemainingOperationScreenOperand{
                        name: name_right, amount: amount_right, illustration: illustration_right
                    }],
                illustration_fields: vec!["Default amount".to_string()],
                archived_operand_with_non_zero_amounts: vec![]
            };
            assert_eq!(result, expected)
        }


        #[test]
        fn two_operands_one_archived_one_month_ago() {
            let date = NaiveDate::from_ymd_opt(2026,5, 9).expect("Can create date");
            let ex = ExchangeRates::for_tests();

            let operand_archived = Operand {
                name: "foo".to_string().clone(),
                amount: ex.yen("0").clone().clone(),
                illustration: vec![
                    ("Default amount".to_string(), IllustrationValue::Amount(ex.yen("300")))
                ].clone(),
                archived_from: Some(
                    date.checked_sub_months(Months::new(1)).expect("can add a month to the date")),
            };

            let name = "baz".to_string();
            let amount = ex.yen("1000").clone();
            let illustration = vec![
                ("Default amount".to_string(), IllustrationValue::Amount(ex.yen("1300")))
            ];
            let operand = Operand {
                name: name.clone(),
                amount: amount.clone(),
                illustration: illustration.clone(),
                archived_from: None,
            };

            let group_name = "bar";

            let group = Group::new(group_name, vec![operand_archived.clone(), operand.clone()])
                .expect("Can create group");
            let result = group
                .into_remaining_operation_screen_group(&ex, &"JPY".to_string(), &date)
                .expect("can build RemainingOperationScreenGroup");
            let expected = RemainingOperationScreenGroup {
                name: group_name.to_string(),
                total: operand_archived.amount + operand.amount,
                operands: vec![
                    RemainingOperationScreenOperand{
                        name, amount, illustration
                    }],
                illustration_fields: vec!["Default amount".to_string()],
                archived_operand_with_non_zero_amounts: vec![]
            };
            assert_eq!(result, expected)
        }

        #[test]
        fn two_operands_one_archived_today() {
            let date = NaiveDate::from_ymd_opt(2026,5, 9).expect("Can create date");
            let ex = ExchangeRates::for_tests();

            let operand_archived = Operand {
                name: "foo".to_string().clone(),
                amount: ex.yen("0").clone().clone(),
                illustration: vec![
                    ("Default amount".to_string(), IllustrationValue::Amount(ex.yen("300")))
                ].clone(),
                archived_from: Some(date.clone())
            };

            let name = "baz".to_string();
            let amount = ex.yen("1000").clone();
            let illustration = vec![
                ("Default amount".to_string(), IllustrationValue::Amount(ex.yen("1300")))
            ];
            let operand = Operand {
                name: name.clone(),
                amount: amount.clone(),
                illustration: illustration.clone(),
                archived_from: None,
            };

            let group_name = "bar";

            let group = Group::new(group_name, vec![operand_archived.clone(), operand.clone()])
                .expect("Can create group");
            let result = group
                .into_remaining_operation_screen_group(&ex, &"JPY".to_string(), &date)
                .expect("can build RemainingOperationScreenGroup");
            let expected = RemainingOperationScreenGroup {
                name: group_name.to_string(),
                total: operand_archived.amount + operand.amount,
                operands: vec![
                    RemainingOperationScreenOperand{
                        name, amount, illustration
                    }],
                illustration_fields: vec!["Default amount".to_string()],
                archived_operand_with_non_zero_amounts: vec![]
            };
            assert_eq!(result, expected)
        }

        #[test]
        fn two_operands_one_archived_today_and_non_zero_amount() {
            let date = NaiveDate::from_ymd_opt(2026,5, 9).expect("Can create date");
            let ex = ExchangeRates::for_tests();

            let archive_operator_name = "foo".to_string().clone();
            let operand_archived = Operand {
                name: archive_operator_name.clone(),
                amount: ex.yen("500").clone().clone(),
                illustration: vec![
                    ("Default amount".to_string(), IllustrationValue::Amount(ex.yen("300")))
                ].clone(),
                archived_from: Some(date.clone())
            };

            let name = "baz".to_string();
            let amount = ex.yen("1000").clone();
            let illustration = vec![
                ("Default amount".to_string(), IllustrationValue::Amount(ex.yen("1300")))
            ];
            let operand = Operand {
                name: name.clone(),
                amount: amount.clone(),
                illustration: illustration.clone(),
                archived_from: None,
            };

            let group_name = "bar";

            let group = Group::new(group_name, vec![operand_archived.clone(), operand.clone()])
                .expect("Can create group");
            let result = group
                .into_remaining_operation_screen_group(&ex, &"JPY".to_string(), &date)
                .expect("can build RemainingOperationScreenGroup");
            let expected = RemainingOperationScreenGroup {
                name: group_name.to_string(),
                total: operand_archived.amount + operand.amount,
                operands: vec![
                    RemainingOperationScreenOperand{
                        name, amount, illustration
                    }],
                illustration_fields: vec!["Default amount".to_string()],
                archived_operand_with_non_zero_amounts: vec![archive_operator_name]
            };
            assert_eq!(result, expected)
        }
    }

    mod integration_test {
        use crate::remaining_operation::core_types::{IllustrationValue, RemainingOperationScreen, RemainingOperationScreenOperand};
        use chrono::NaiveDate;
        use rust_decimal::Decimal;
        use rust_decimal_macros::dec;
        use crate::accounts::{AccountJson};
        use crate::buckets::Bucket;
        use crate::ignored_transaction::{IgnoredTransactionBuilder};
        use crate::period::{CalendarMonthPeriodConfiguration, Period, PeriodConfigurationVaultValue, PeriodsConfiguration};
        use crate::predicted_income::{PredictedIncomeBuilder};
        use crate::amounts::Amount;
        use crate::amounts::exchange_rates::ExchangeRates;
        use crate::remaining_operation::core_types::{GroupBuilder, Operand, OperandBuilder, RemainingOperation, RemainingOperationScreenGroup};
        use pretty_assertions::assert_eq;
        use serde_json::{from_value, json};
        use crate::remaining_operation::core_types::group::Group;

        struct TestGroupBuilder<OB: OperandBuilder> {
            name: String,
            operand_builders: Vec<OB>,
        }

        impl<OB: OperandBuilder> GroupBuilder<OB> for TestGroupBuilder<OB> {
            fn build(self) -> Result<(String, Vec<OB>), String> {
                Ok((self.name, self.operand_builders))
            }
        }

        #[test]
        fn test() {
            // Exchange rate is one euro for 2 yens
            //
            // Two accounts in euros
            // Two accounts in yen
            //
            // Two goals (one for which things have been committed, one for which things remain to be committed)
            //
            // Two ignored transaction (one in each currency)
            // One predicted income
            //
            // Check that everything is in its place and the the remaining operation is correct

            fn mkdate(month: u32, date: u32) -> NaiveDate {
                NaiveDate::from_ymd_opt(2023, month, date).expect("Can create date")
            }

            let period_configuration = PeriodConfigurationVaultValue::CalendarMonth(CalendarMonthPeriodConfiguration {});
            let today = mkdate(8, 20);
            let exchange_rates = ExchangeRates::for_tests();


            let mut remaining_operation = RemainingOperation::new(
                period_configuration,
                today,
                exchange_rates.clone(),
            );

            let account_euro_left = AccountJson::new(
                "account in euros left".to_string(),
                "EUR".to_string(),
                vec![
                    (mkdate(7, 1), 1000),
                    (mkdate(8, 2), 1500),
                    (mkdate(8, 3), 2200),
                ],
                None
            );

            let account_euro_right = AccountJson::new(
                "account in euros right".to_string(),
                "EUR".to_string(),
                vec![
                    (mkdate(7, 15), 500),
                    (mkdate(8, 2), 500),
                    (mkdate(8, 3), 300),
                ],
                None
            );

            let account_yen_left = AccountJson::new(
                "account in yen left".to_string(),
                "JPY".to_string(),
                vec![
                    (mkdate(7, 31), 500),
                ],
                None
            );

            let account_yen_right = AccountJson::new(
                "account in yen right".to_string(),
                "JPY".to_string(),
                vec![
                    (mkdate(7, 2), 700),
                    (mkdate(8, 15), 700),
                ],
                None
            );

            let account_archived = AccountJson::new(
                "account archived".to_string(),
                "EUR".to_string(),
                vec![
                    (mkdate(5, 20), 1000),
                    (mkdate(5, 30), 700)
                ],
                Some(mkdate(6,1))
            );

            let accounts = TestGroupBuilder {
                name: "Accounts".into(),
                operand_builders: vec![
                    account_euro_left,
                    account_euro_right,
                    account_yen_left,
                    account_yen_right,
                    account_archived,
                ],
            };
            remaining_operation.add_group(accounts).expect("Can add accounts");

            let bucket_must_commit: Bucket = from_value(json!({
                "name": "Goal must commit",
                "lines": [
                    "2023/07/01 TARG ¥200 2023/08/31",
                    "2023/07/18 DEPO ¥150"
                ]
            })).expect("Can deserialize bucket");

            let bucket_already_committed: Bucket = from_value(json!({
                "name": "Goal already committed",
                "lines": [
                    "2023/07/01 TARG ¥500 2023/08/31",
                    "2023/07/18 DEPO ¥100",
                    "2023/08/17 DEPO ¥100"
                ]
            })).expect("Can deserialize bucket");

            let bucket_archived_but_affecting_result: Bucket = from_value(json!({
                "name": "Archived bucket",
                "lines": [
                    "2023/08/19 DEPO ¥100"
                ],
                "archived_since": "2023-08-20"
            })).expect("Can deserialize bucket");

            let goals = TestGroupBuilder {
                name: "Buckets".into(),
                operand_builders: vec![
                    bucket_must_commit,
                    bucket_already_committed,
                    bucket_archived_but_affecting_result
                ],
            };
            remaining_operation.add_group(goals).expect("Can add goals");

            let ignored_incoming = IgnoredTransactionBuilder::default()
                .name("Ignored incoming".to_string())
                .currency("EUR".to_string())
                .amount(dec!(200))
                .date(mkdate(8, 15))
                .build()
                .expect("Can build ignored transaction");

            let ignored_outgoing = IgnoredTransactionBuilder::default()
                .name("Ignored outgoing".to_string())
                .currency("JPY".to_string())
                .amount(dec!(-800))
                .date(mkdate(8, 14))
                .build()
                .expect("Can build ignored transaction");

            let ignored_later_this_month = IgnoredTransactionBuilder::default()
                .name("Ignored later this month".to_string())
                .currency("EUR".to_string())
                .amount(dec!(200))
                .date(mkdate(8, 21))
                .build()
                .expect("Can build ignored transaction");

            let ignored_last_month = IgnoredTransactionBuilder::default()
                .name("Ignored last month".to_string())
                .currency("EUR".to_string())
                .amount(dec!(200))
                .date(mkdate(7, 21))
                .build()
                .expect("Can build ignored transaction");

            let ignored_transaction = TestGroupBuilder {
                name: "Ignored transactions".to_string(),
                operand_builders: vec![
                    ignored_incoming,
                    ignored_outgoing,
                    ignored_later_this_month,
                    ignored_last_month,
                ],
            };
            remaining_operation.add_group(ignored_transaction).expect("Can add ignored transactions");

            let predicted_income = PredictedIncomeBuilder::default()
                .currency("JPY".to_string())
                .figure(dec!(400))
                .build()
                .expect("Can build predicted income");

            let predicted_incomes = TestGroupBuilder {
                name: "Predicted Income".to_string(),
                operand_builders: vec![
                    predicted_income
                ],
            };

            remaining_operation.add_group(predicted_incomes).expect("Can add predicted incomes");

            let result_eur = remaining_operation.execute(&"EUR".to_string()).expect("Can execute remaining operation for yens");

            assert_eq!(
                result_eur,
                RemainingOperationScreen {
                    remaining: exchange_rates.euro("875.00"),
                    period: Period {
                        start_date: mkdate(8, 1),
                        end_date: mkdate(8, 31),
                    },
                    groups: vec![
                        RemainingOperationScreenGroup {
                            name: "Accounts".into(),
                            operands: vec![
                                RemainingOperationScreenOperand {
                                    name: "account in euros left".to_string(),
                                    amount: exchange_rates.euro("1200"),
                                    illustration: vec![
                                        ("Period start amount".into(), IllustrationValue::Amount(exchange_rates.euro("1000"))),
                                        ("Period end amount".into(), IllustrationValue::Amount(exchange_rates.euro("2200"))),
                                        ("Committed".into(), IllustrationValue::Bool(true)),
                                        ("Difference".into(), IllustrationValue::Amount(exchange_rates.euro("1200"))),
                                    ],
                                },
                                RemainingOperationScreenOperand {
                                    name: "account in euros right".to_string(),
                                    amount: exchange_rates.euro("-200"),
                                    illustration: vec![
                                        ("Period start amount".into(), IllustrationValue::Amount(exchange_rates.euro("500"))),
                                        ("Period end amount".into(), IllustrationValue::Amount(exchange_rates.euro("300"))),
                                        ("Committed".into(), IllustrationValue::Bool(true)),
                                        ("Difference".into(), IllustrationValue::Amount(exchange_rates.euro("-200"))),
                                    ],
                                },
                                RemainingOperationScreenOperand {
                                    name: "account in yen left".to_string(),
                                    amount: exchange_rates.yen("0"),
                                    illustration: vec![
                                        ("Period start amount".into(), IllustrationValue::Amount(exchange_rates.yen("500"))),
                                        ("Period end amount".into(), IllustrationValue::Amount(exchange_rates.yen("500"))),
                                        ("Committed".into(), IllustrationValue::Bool(true)),
                                        ("Difference".into(), IllustrationValue::Amount(exchange_rates.yen("0"))),
                                    ],
                                },
                                RemainingOperationScreenOperand {
                                    name: "account in yen right".to_string(),
                                    amount: exchange_rates.yen("0"),
                                    illustration: vec![
                                        ("Period start amount".into(), IllustrationValue::Amount(exchange_rates.yen("700"))),
                                        ("Period end amount".into(), IllustrationValue::Amount(exchange_rates.yen("700"))),
                                        ("Committed".into(), IllustrationValue::Bool(true)),
                                        ("Difference".into(), IllustrationValue::Amount(exchange_rates.yen("0"))),
                                    ],
                                },
                            ],
                            illustration_fields: vec!["Period start amount".into(), "Period end amount".into(), "Committed".into(), "Difference".into()],
                            total: exchange_rates.euro("1000.00"),
                            archived_operand_with_non_zero_amounts: vec![]
                        },
                        RemainingOperationScreenGroup {
                            name: "Buckets".into(),
                            operands: vec![
                                RemainingOperationScreenOperand {
                                    name: "Goal must commit".to_string(),
                                    amount: exchange_rates.yen("-50"),
                                    illustration: vec![
                                        ("This period - recommended deposit".into(), IllustrationValue::Amount(exchange_rates.yen("50"))),
                                        ("This period - actual deposit".into(), IllustrationValue::Null),
                                        ("This period - actual withdrawal".into(), IllustrationValue::Null),
                                        ("Deposited".into(), IllustrationValue::Amount(exchange_rates.yen("150"))),
                                        ("Withdrawn".into(), IllustrationValue::Amount(exchange_rates.yen("0"))),
                                        ("Total".into(), IllustrationValue::Amount(exchange_rates.yen("150"))),
                                    ]
                                },
                                RemainingOperationScreenOperand {
                                    name: "Goal already committed".to_string(),
                                    amount: exchange_rates.yen("-100"),
                                    illustration: vec![
                                        ("This period - recommended deposit".into(), IllustrationValue::Amount(exchange_rates.yen("400"))),
                                        ("This period - actual deposit".into(), IllustrationValue::Amount(exchange_rates.yen("100"))),
                                        ("This period - actual withdrawal".into(), IllustrationValue::Null),
                                        ("Deposited".into(), IllustrationValue::Amount(exchange_rates.yen("200"))),
                                        ("Withdrawn".into(), IllustrationValue::Amount(exchange_rates.yen("0"))),
                                        ("Total".into(), IllustrationValue::Amount(exchange_rates.yen("200"))),
                                    ]
                                },
                            ],
                            illustration_fields: vec![
                                "This period - recommended deposit".into(),
                                "This period - actual deposit".into(),
                                "This period - actual withdrawal".into(),
                                "Deposited".into(),
                                "Withdrawn".into(),
                                "Total".into(),
                            ],
                            total: exchange_rates.euro("-125.00"),
                            archived_operand_with_non_zero_amounts: vec!["Archived bucket".to_string()]
                        },
                        RemainingOperationScreenGroup {
                            name: "Ignored transactions".into(),
                            operands: vec![
                                RemainingOperationScreenOperand {
                                    name: "Ignored incoming".to_string(),
                                    amount: exchange_rates.euro("200"),
                                    illustration: vec![
                                        ("Included".to_string(), IllustrationValue::Bool(true)),
                                        ("Date".to_string(), IllustrationValue::Date(mkdate(8, 15)))
                                    ]
                                },
                                RemainingOperationScreenOperand {
                                    name: "Ignored outgoing".to_string(),
                                    amount: exchange_rates.yen("-800"),
                                    illustration: vec![
                                        ("Included".to_string(), IllustrationValue::Bool(true)),
                                        ("Date".to_string(), IllustrationValue::Date(mkdate(8, 14)))
                                    ]
                                },
                                RemainingOperationScreenOperand {
                                    name: "Ignored later this month".to_string(),
                                    amount: exchange_rates.euro("0"),
                                    illustration: vec![
                                        ("Included".to_string(), IllustrationValue::Bool(false)),
                                        ("Date".to_string(), IllustrationValue::Date(mkdate(8, 21)))
                                    ]
                                },
                            ],
                            illustration_fields: vec!["Included".into(), "Date".into()],
                            total: exchange_rates.euro("-200.00"),
                            archived_operand_with_non_zero_amounts: vec![]
                        },
                        RemainingOperationScreenGroup {
                            name: "Predicted Income".into(),
                            operands: vec![RemainingOperationScreenOperand {
                                name: "Predicted Income".to_string(),
                                amount: exchange_rates.yen("400"),
                                illustration: vec![],
                            }],
                            illustration_fields: vec![],
                            total: exchange_rates.euro("200.00"),
                            archived_operand_with_non_zero_amounts: vec![]
                        }
                    ],
                }
            );

            let result_jpy = remaining_operation.execute(&"JPY".to_string()).expect("Can execute remaining operation for yens");
            assert_eq!(result_jpy.groups.iter().map(|g| g.total.clone()).collect::<Vec<Amount>>(), vec![
                exchange_rates.yen("2000"),
                exchange_rates.yen("-250"),
                exchange_rates.yen("-400"),
                exchange_rates.yen("400")
            ]);

            assert_eq!(result_jpy.remaining, exchange_rates.yen("1750"));
        }
    }
}
