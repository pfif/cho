use super::amounts::exchange_rates::ExchangeRates;
use super::amounts::{Amount, CurrencyIdent};
use crate::period::{Period, PeriodConfigurationVaultValue, PeriodsConfiguration};
use crate::goals::{GoalVaultValues};
use chrono::{Local, NaiveDate};
use group::Group;
use rust_decimal_macros::dec;
use crate::accounts::AccountGetter;
use crate::ignored_transaction::IgnoredTransactionsVaultValues;
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
            exchange_rates
        );
        operation.add_group(AccountGetter::from_files(vault)?)?;
        operation.add_group(GoalVaultValues::from_vault(vault)?)?;
        operation.add_group(IgnoredTransactionsVaultValues::from_vault(vault)?)?;
        Ok(operation)
        /*let mut groups: Vec<Group> = vec![
            IgnoredTransactionsVaultValues::from_vault(vault)?.into(),
        ];
        if include_predicted_income {
            group_factories.push(PredictedIncome::from_vault(vault)?.into());
        }
        Ok(
            RemainingOperation{
                date: Local::now().date_naive(),
                periods_configuration: ,
                group_factories
            }
        )*/
    }

    pub fn add_group<B: GroupBuilder>(&mut self, builder: B) -> Result<(), String> {
        let group = builder.build(&self.periods_configuration, &self.date, &self.exchange_rates)?;
        self.groups.push(group);
        Ok(())
    }
}
impl RemainingOperation {
    pub fn execute(
        &self,
        target_currency: &CurrencyIdent,
    ) -> Result<RemainingOperationScreen, String> {

        let current_period = self
            .periods_configuration
            .period_for_date(&self.date)
            .map_err(|error| "Failed to fetch Periods Configuration: ".to_string() + &error)?;

        let mut remaining: Amount = self.exchange_rates.new_amount(&target_currency, dec!(0))?;
        for group in self.groups.iter() {
            for operand in group.operands() {
                remaining = &remaining + &operand.amount
            }
        }

        Ok(RemainingOperationScreen {
            period: current_period,
            groups: self.groups.clone(),
            remaining,
        })
    }
}

/* Builders */
pub trait GroupBuilder {
    fn build(
        self,
        period_configuration: &PeriodConfigurationVaultValue,
        today: &NaiveDate,
        // TODO Remove exchange rate from this interface (see comment at OperandBuilder.build)
        exchange_rates: &ExchangeRates,
    ) -> Result<Group, String>;
}

pub trait OperandBuilder {
    fn build(
        self,
        period_configuration: &PeriodConfigurationVaultValue,
        today: &NaiveDate,
        // Exchange rate is only necessary because other parts of the codebase need to convert their understanding of currency into Amounts produced by Exchange rates
        // Once the entire codebase adopts ExchangeRates, we won't need to pass it around
        // TODO Remove exchange rate from this interface
        exchange_rates: &ExchangeRates,
    ) -> Result<Option<Operand>, String>;
}

/* Output types */
pub struct RemainingOperationScreen {
    pub groups: Vec<Group>,
    pub remaining: Amount,
    pub period: Period,
}

// The struct Group has its own module to isolate its internal attribute
pub mod group {
    use chrono::NaiveDate;
    use crate::period::PeriodConfigurationVaultValue;
    use crate::remaining_operation::amounts::exchange_rates::ExchangeRates;
    use super::{Operand, OperandBuilder};

    /* TODO - we're also not returning the total of each group */
    #[derive(Clone)]
    pub struct Group {
        name: String,
        operands: Vec<Operand>,
        illustration_fields: Option<Vec<String>>,
    }

    impl Group {
        pub(crate) fn new(name: &str) -> Group {
            Group {
                name: name.to_string(),
                operands: vec![],
                illustration_fields: None,
            }
        }
        pub fn add_operands_through_builder<B: OperandBuilder>(&mut self, operand_builder: B, period_configuration: &PeriodConfigurationVaultValue, today: &NaiveDate, exchange_rates: &ExchangeRates) -> Result<(), String> {
            operand_builder.build(period_configuration, today, exchange_rates)?
                .map_or(Ok(()), |operand| self.add_operands(operand))
        }

        pub fn add_operands(&mut self, o: Operand) -> Result<(), String> {
            if self.illustration_fields == None {
                let fields = o.illustration.iter().map(|(k, _)| k.clone()).collect();
                self.illustration_fields = Some(fields)
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
        // TODO - Not a fan that, if there is a bug in `add_operands`, operands could be returned
        //        with an irregular number of column, or with column not matching the illustration
        //        fields
        pub fn operands(&self) -> &Vec<Operand> {
            &self.operands
        }
        pub fn illustration_fields(&self) -> Vec<String> {
            self.illustration_fields.clone().unwrap_or_else(|| vec![])
        }
        
        pub fn empty(&self) -> bool {
            self.operands.is_empty()
        }
    }
}

// TODO Problem with IlLustrationValue - they exists to split logic from view - we don't dictate
//      how to show the value, we just return a type
//      However, this feels incompatible with not returning a type of Illustration, and instead
//      just a list of columns
//      Downstream code can decide how to display the value, but not only based on its type, not
//      what it _is_
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IllustrationValue {
    Amount(Amount),
    Bool(bool),
    Date(NaiveDate),
}

pub type Illustration = Vec<(String, IllustrationValue)>;

#[derive(Debug, Clone)]
#[derive(PartialEq)]
pub struct Operand {
    pub name: String,
    pub amount: Amount,
    pub illustration: Illustration,
}

#[cfg(test)]
mod test {
    use crate::remaining_operation::core_types::IllustrationValue;
use chrono::NaiveDate;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use crate::accounts::{AccountJson};
    use crate::goals::{GoalImplementationBuilder};
    use crate::ignored_transaction::{IgnoredTransactionBuilder};
    use crate::period::{CalendarMonthPeriodConfiguration, Period, PeriodConfigurationVaultValue, PeriodsConfiguration};
    use crate::predicted_income::{PredictedIncomeBuilder};
    use crate::remaining_operation::amounts::exchange_rates::ExchangeRates;
    use crate::remaining_operation::core_types::{GroupBuilder, Operand, OperandBuilder, RemainingOperation};
    use crate::remaining_operation::core_types::group::Group;

    struct TestGroupBuilder<OB: OperandBuilder> {
        name: String,
        operand_builders: Vec<OB>
    }

    impl<OB: OperandBuilder> GroupBuilder for TestGroupBuilder<OB> {
        fn build(self, period_configuration: &PeriodConfigurationVaultValue, today: &NaiveDate, exchange_rates: &ExchangeRates) -> Result<Group, String> {
            let mut group = Group::new(&self.name);
            for operand in self.operand_builders{
                group.add_operands_through_builder(operand, period_configuration, today, exchange_rates)?;
            }
            Ok(group)
        }
    }

    #[test]
    fn test(){
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

        let period_configuration = PeriodConfigurationVaultValue::CalendarMonth(CalendarMonthPeriodConfiguration{});
        let today = mkdate(8, 20);
        let exchange_rates = ExchangeRates::from_indent_and_rates(vec![
            ("EUR".to_string(), dec!(1)),
            ("JPY".to_string(), dec!(2))
        ]).expect("Can create exchange rates");

        let price_e = |amount: i16| {
            exchange_rates.new_amount(&"EUR".to_string(), Decimal::from(amount)).expect("Can create amount")
        };

        let price_j = |amount: i16| {
            exchange_rates.new_amount(&"JPY".to_string(), Decimal::from(amount)).expect("Can create amount")
        };

        let amount_timeline = |name: &str, start: i16, end: i16 , amount: i16| {
            Operand{
                name: "account in euros left".to_string(),
                amount: price_e(1200),
                illustration: vec![
                    ("Period start amount".into(), IllustrationValue::Amount(price_e(1000))),
                    ("Period end amount".into(), IllustrationValue::Amount(price_e(2200))),
                    ("Committed".into(), IllustrationValue::Bool(true)),
                    ("Difference".into(), IllustrationValue::Amount(price_e(1200))),
                ],
            }
        };

        let mut remaining_operation = RemainingOperation::new(
            period_configuration,
            today,
            exchange_rates.clone()
        );

        let account_euro_left = AccountJson::new(
                "account in euros left".to_string(),
                "EUR".to_string(),
                vec![
                    (mkdate(7, 1), 1000),
                    (mkdate(8, 2), 1500),
                    (mkdate(8, 3), 2200),
                ]
            );

        let account_euro_right = AccountJson::new(
            "account in euros right".to_string(),
            "EUR".to_string(),
            vec![
                (mkdate(7, 15), 500),
                (mkdate(8, 2), 500),
                (mkdate(8, 3), 300),
            ]
        );

        let account_yen_left = AccountJson::new(
            "account in yen left".to_string(),
            "JPY".to_string(),
            vec![
                (mkdate(7, 31), 500),
            ]
        );

        let account_yen_right = AccountJson::new(
            "account in yen right".to_string(),
            "JPY".to_string(),
            vec![
                (mkdate(7, 2), 700),
                (mkdate(8, 15), 700),
            ]
        );


        let accounts = TestGroupBuilder{
            name: "Accounts".into(),
            operand_builders: vec![
                account_euro_left,
                account_euro_right,
                account_yen_left,
                account_yen_right,
            ]
        };
        remaining_operation.add_group(accounts).expect("Can add accounts");

        let goal_must_commit = GoalImplementationBuilder::default()
            .name("Goal must commit".to_string())
            .currency("EUR".to_string())
            .target(dec!(200))
            .target_date(mkdate(8, 31))
            .committed(vec![
                (mkdate(7, 18), dec!(150)),
            ])
            .build()
            .expect("Can build goal");

        let goal_already_committed = GoalImplementationBuilder::default()
            .name("Goal already committed".to_string())
            .currency("EUR".to_string())
            .target(dec!(500))
            .target_date(mkdate(8, 31))
            .committed(vec![
                (mkdate(7, 18), dec!(100)),
                (mkdate(8, 17), dec!(100)),
            ])
            .build()
            .expect("Can build goal");

        let goals = TestGroupBuilder{
            name: "Goals".into(),
            operand_builders: vec![
                goal_must_commit,
                goal_already_committed,
            ]
        };
        remaining_operation.add_group(goals).expect("Can add goals");

        let ignored_incoming = IgnoredTransactionBuilder::default()
            .name("Ignore incoming".to_string())
            .currency("EUR".to_string())
            .amount(dec!(200))
            .date(mkdate(8, 15))
            .build()
            .expect("Can build ignored transaction");

        let ignored_outgoing = IgnoredTransactionBuilder::default()
            .name("Ignore outgoing".to_string())
            .currency("JPY".to_string())
            .amount(dec!(-400))
            .date(mkdate(8, 14))
            .build()
            .expect("Can build ignored transaction");

        let ignored_transaction = TestGroupBuilder {
           name: "Ignored transactions".to_string(),
           operand_builders: vec![
               ignored_incoming,
               ignored_outgoing
           ]
        };
        remaining_operation.add_group(ignored_transaction).expect("Can add ignored transactions");

        let predicted_income = PredictedIncomeBuilder::default()
            .currency("JPY".to_string())
            .figure(dec!(350))
            .build()
            .expect("Can build predicted income");

        let predicted_incomes = TestGroupBuilder {
            name: "Predicted Incomes".to_string(),
            operand_builders: vec![
                predicted_income
            ]
        };

        remaining_operation.add_group(predicted_incomes).expect("Can add predicted incomes");

        let result = remaining_operation.execute(&"EUR".to_string()).expect("Can execute remaining operation for yens");

        let period = Period {
            start_date: mkdate(8, 1),
            end_date: mkdate(8, 31),
        };

        assert_eq!(result.period, period);

        let group_account = result.groups[0].clone();
        assert_eq!(group_account.name(), "Accounts");

        assert_eq!(group_account.operands(), &vec![
            Operand {
                name: "account in euros left".to_string(),
                amount: price_e(1200),
                illustration: vec![
                    ("Period start amount".into(), IllustrationValue::Amount(price_e(1000))),
                    ("Period end amount".into(), IllustrationValue::Amount(price_e(2200))),
                    ("Committed".into(), IllustrationValue::Bool(true)),
                    ("Difference".into(), IllustrationValue::Amount(price_e(1200))),
                ],
            },
            Operand {
                name: "account in euros right".to_string(),
                amount: price_e(-200),
                illustration: vec![
                    ("Period start amount".into(), IllustrationValue::Amount(price_e(500))),
                    ("Period end amount".into(), IllustrationValue::Amount(price_e(300))),
                    ("Committed".into(), IllustrationValue::Bool(true)),
                    ("Difference".into(), IllustrationValue::Amount(price_e(-200))),
                ],
            },
            Operand {
                name: "account in yen left".to_string(),
                amount: price_j(0),
                illustration: vec![
                    ("Period start amount".into(), IllustrationValue::Amount(price_j(500))),
                    ("Period end amount".into(), IllustrationValue::Amount(price_j(500))),
                    ("Committed".into(), IllustrationValue::Bool(true)),
                    ("Difference".into(), IllustrationValue::Amount(price_j(0))),
                ],
            },
            Operand {
                name: "account in yen right".to_string(),
                amount: price_j(0),
                illustration: vec![
                    ("Period start amount".into(), IllustrationValue::Amount(price_j(700))),
                    ("Period end amount".into(), IllustrationValue::Amount(price_j(700))),
                    ("Committed".into(), IllustrationValue::Bool(true)),
                    ("Difference".into(), IllustrationValue::Amount(price_j(0))),
                ],
            },
        ]);

        let group_goals = result.groups[1].clone();
        assert_eq!(group_goals.name(), "Goals");

        assert_eq!(group_goals.operands(), &vec![
            Operand{
                name: "Goal must commit".to_string(),
                amount: price_e(50),
                illustration: vec![
                    ("Amount".into(), IllustrationValue::Amount(price_e(50))),
                    ("Committed".into(), IllustrationValue::Amount(price_e(150))),
                    ("Payed in".into(), IllustrationValue::Bool(false)),
                    ("Target".into(), IllustrationValue::Amount(price_e(200))),
                ]
            },
            Operand{
                name: "Goal already committed".to_string(),
                amount: price_e(100),
                illustration: vec![
                    ("Amount".into(), IllustrationValue::Amount(price_e(100))),
                    ("Committed".into(), IllustrationValue::Amount(price_e(200))),
                    ("Payed in".into(), IllustrationValue::Bool(true)),
                    ("Target".into(), IllustrationValue::Amount(price_e(500))),
                ]
            },
        ])
    }
}
