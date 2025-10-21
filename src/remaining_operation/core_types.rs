use crate::amounts::exchange_rates::ExchangeRates;
use crate::amounts::{Amount, CurrencyIdent};
use crate::period::{Period, PeriodConfigurationVaultValue, PeriodsConfiguration};
use crate::goals::{GoalVaultValues};
use chrono::{Local, NaiveDate};
use group::Group;
use rust_decimal_macros::dec;
use crate::accounts::AccountGetter;
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
        operation.add_group(GoalVaultValues::from_vault(vault)?)?;
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

        let remaining_operation_screen_group = self.groups
            .clone()
            .into_iter()
            .map(|group| group.into_remaining_operation_screen_group(
                &self.exchange_rates, target_currency))
            .collect::<Result<Vec<RemainingOperationScreenGroup>, String>>()?;

        let mut remaining: Amount = self.exchange_rates.new_amount(target_currency, dec!(0))?;
        for group in self.groups.iter() {
            for operand in group.operands() {
                remaining = remaining.add(&operand.amount)
            }
        }

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
#[derive(PartialEq, Debug, Eq)]
pub struct RemainingOperationScreen {
    pub groups: Vec<RemainingOperationScreenGroup>,
    pub remaining: Amount,
    pub period: Period,
}

#[derive(PartialEq, Debug, Eq)]
pub struct RemainingOperationScreenGroup {
    pub name: String,
    pub operands: Vec<Operand>,
    pub illustration_fields: Vec<String>,
    pub total: Amount
}

impl RemainingOperationScreenGroup {
    pub fn empty(&self) -> bool {
        self.operands.is_empty()
    }
}

// The struct Group has its own module to isolate its internal attribute
pub mod group {
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;
    use crate::period::PeriodConfigurationVaultValue;
    use crate::amounts::{Amount, Currency, CurrencyIdent};
    use crate::amounts::exchange_rates::ExchangeRates;
    use super::{GroupBuilder, Operand, OperandBuilder, RemainingOperationScreenGroup};

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

        pub fn from_group_builder<O: OperandBuilder, B: GroupBuilder<O>>(
            group_builder: B,
            exchange_rates: &ExchangeRates,
            period_configuration: &PeriodConfigurationVaultValue,
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
        ) -> Result<RemainingOperationScreenGroup, String> {
            let total = self.operands
                .iter()
                .fold(exchange_rates.new_amount(target_currency, dec!(0))?, |acc, operand| {
                    acc.add(&operand.amount)
                });

           Ok(RemainingOperationScreenGroup{
               name: self.name,
               operands: self.operands,
               illustration_fields: self.illustration_fields.unwrap_or(vec![]),
               total
           })
        }
    }

    #[cfg(test)]
    impl Group {
        /// Create a group initialized with any internals. Allows to create a group with invalid internal state
        pub fn from_internals(name: &str, operands: Vec<Operand>, illustration_fields: Vec<&str>) -> Group {
            Group{
                name: name.to_string(),
                operands,
                illustration_fields: Some(illustration_fields
                    .into_iter()
                    .map(|field| field.to_string())
                    .collect()),
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IllustrationValue {
    Amount(Amount),
    NullAmount,
    Bool(bool),
    Date(NaiveDate),
}

impl From<Amount> for IllustrationValue {
    fn from(value: Amount) -> Self {
        IllustrationValue::Amount(value)
    }
}

impl From<Option<Amount>> for IllustrationValue {
    fn from(value: Option<Amount>) -> Self {
       value
           .map(|amount| amount.into())
           .unwrap_or(IllustrationValue::NullAmount)
    }
}

pub type Illustration = Vec<(String, IllustrationValue)>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operand {
    pub name: String,
    pub amount: Amount,
    pub illustration: Illustration,
}

#[cfg(test)]
mod test {
    use crate::remaining_operation::core_types::{IllustrationValue, RemainingOperationScreen};
    use chrono::NaiveDate;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use crate::accounts::{AccountJson};
    use crate::goals::{GoalBuilder};
    use crate::ignored_transaction::{IgnoredTransactionBuilder};
    use crate::period::{CalendarMonthPeriodConfiguration, Period, PeriodConfigurationVaultValue, PeriodsConfiguration};
    use crate::predicted_income::{PredictedIncomeBuilder};
    use crate::amounts::Amount;
    use crate::amounts::exchange_rates::ExchangeRates;
    use crate::remaining_operation::core_types::{GroupBuilder, Operand, OperandBuilder, RemainingOperation, RemainingOperationScreenGroup};
    use pretty_assertions::assert_eq;
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
        );

        let account_euro_right = AccountJson::new(
            "account in euros right".to_string(),
            "EUR".to_string(),
            vec![
                (mkdate(7, 15), 500),
                (mkdate(8, 2), 500),
                (mkdate(8, 3), 300),
            ],
        );

        let account_yen_left = AccountJson::new(
            "account in yen left".to_string(),
            "JPY".to_string(),
            vec![
                (mkdate(7, 31), 500),
            ],
        );

        let account_yen_right = AccountJson::new(
            "account in yen right".to_string(),
            "JPY".to_string(),
            vec![
                (mkdate(7, 2), 700),
                (mkdate(8, 15), 700),
            ],
        );


        let accounts = TestGroupBuilder {
            name: "Accounts".into(),
            operand_builders: vec![
                account_euro_left,
                account_euro_right,
                account_yen_left,
                account_yen_right,
            ],
        };
        remaining_operation.add_group(accounts).expect("Can add accounts");

        let goal_must_commit = GoalBuilder::default()
            .name("Goal must commit".to_string())
            .currency("EUR".to_string())
            .target(dec!(200))
            .target_date(mkdate(8, 31))
            .committed(vec![
                (mkdate(7, 18), dec!(150)),
            ])
            .build()
            .expect("Can build goal");

        let goal_already_committed = GoalBuilder::default()
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

        let goals = TestGroupBuilder {
            name: "Goals".into(),
            operand_builders: vec![
                goal_must_commit,
                goal_already_committed,
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
                remaining: exchange_rates.euro("850.00"),
                period: Period {
                    start_date: mkdate(8, 1),
                    end_date: mkdate(8, 31),
                },
                groups: vec![
                    RemainingOperationScreenGroup {
                        name: "Accounts".into(),
                        operands: vec![
                            Operand {
                                name: "account in euros left".to_string(),
                                amount: exchange_rates.euro("1200"),
                                illustration: vec![
                                    ("Period start amount".into(), IllustrationValue::Amount(exchange_rates.euro("1000"))),
                                    ("Period end amount".into(), IllustrationValue::Amount(exchange_rates.euro("2200"))),
                                    ("Committed".into(), IllustrationValue::Bool(true)),
                                    ("Difference".into(), IllustrationValue::Amount(exchange_rates.euro("1200"))),
                                ],
                            },
                            Operand {
                                name: "account in euros right".to_string(),
                                amount: exchange_rates.euro("-200"),
                                illustration: vec![
                                    ("Period start amount".into(), IllustrationValue::Amount(exchange_rates.euro("500"))),
                                    ("Period end amount".into(), IllustrationValue::Amount(exchange_rates.euro("300"))),
                                    ("Committed".into(), IllustrationValue::Bool(true)),
                                    ("Difference".into(), IllustrationValue::Amount(exchange_rates.euro("-200"))),
                                ],
                            },
                            Operand {
                                name: "account in yen left".to_string(),
                                amount: exchange_rates.yen("0"),
                                illustration: vec![
                                    ("Period start amount".into(), IllustrationValue::Amount(exchange_rates.yen("500"))),
                                    ("Period end amount".into(), IllustrationValue::Amount(exchange_rates.yen("500"))),
                                    ("Committed".into(), IllustrationValue::Bool(true)),
                                    ("Difference".into(), IllustrationValue::Amount(exchange_rates.yen("0"))),
                                ],
                            },
                            Operand {
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
                        total: exchange_rates.euro("1000.00")
                    },
                    RemainingOperationScreenGroup {
                        name: "Goals".into(),
                        operands: vec![
                            Operand {
                                name: "Goal must commit".to_string(),
                                amount: exchange_rates.euro("-50"),
                                illustration: vec![
                                    ("Committed".into(), IllustrationValue::Amount(exchange_rates.euro("150"))),
                                    ("Payed in".into(), IllustrationValue::Bool(false)),
                                    ("Target".into(), IllustrationValue::Amount(exchange_rates.euro("200"))),
                                ]
                            },
                            Operand {
                                name: "Goal already committed".to_string(),
                                amount: exchange_rates.euro("-100"),
                                illustration: vec![
                                    ("Committed".into(), IllustrationValue::Amount(exchange_rates.euro("200"))),
                                    ("Payed in".into(), IllustrationValue::Bool(true)),
                                    ("Target".into(), IllustrationValue::Amount(exchange_rates.euro("500"))),
                                ]
                            },
                        ],
                        illustration_fields: vec!["Committed".into(), "Payed in".into(), "Target".into()],
                        total: exchange_rates.euro("-150.00")
                    },
                    RemainingOperationScreenGroup {
                        name: "Ignored transactions".into(),
                        operands: vec![
                            Operand {
                                name: "Ignored incoming".to_string(),
                                amount: exchange_rates.euro("200"),
                                illustration: vec![
                                    ("Included".to_string(), IllustrationValue::Bool(true)),
                                    ("Date".to_string(), IllustrationValue::Date(mkdate(8, 15)))
                                ]
                            },
                            Operand {
                                name: "Ignored outgoing".to_string(),
                                amount: exchange_rates.yen("-800"),
                                illustration: vec![
                                    ("Included".to_string(), IllustrationValue::Bool(true)),
                                    ("Date".to_string(), IllustrationValue::Date(mkdate(8, 14)))
                                ]
                            },
                            Operand {
                                name: "Ignored later this month".to_string(),
                                amount: exchange_rates.euro("0"),
                                illustration: vec![
                                    ("Included".to_string(), IllustrationValue::Bool(false)),
                                    ("Date".to_string(), IllustrationValue::Date(mkdate(8, 21)))
                                ]
                            },
                        ],
                        illustration_fields: vec!["Included".into(), "Date".into()],
                        total: exchange_rates.euro("-200.00")
                    },
                    RemainingOperationScreenGroup {
                        name: "Predicted Income".into(),
                        operands: vec![Operand {
                            name: "Predicted Income".to_string(),
                            amount: exchange_rates.yen("400"),
                            illustration: vec![],
                        }],
                        illustration_fields: vec![],
                        total: exchange_rates.euro("200.00")
                    }
                ],
            }
        );
        let result_jpy = remaining_operation.execute(&"JPY".to_string()).expect("Can execute remaining operation for yens");
        assert_eq!(result_jpy.groups.iter().map(|g| g.total.clone()).collect::<Vec<Amount>>(), vec![
            exchange_rates.yen("2000"),
            exchange_rates.yen("-300"),
            exchange_rates.yen("-400"),
            exchange_rates.yen("400")
        ]);
        
        assert_eq!(result_jpy.remaining, exchange_rates.yen("1700"));
    }
}
