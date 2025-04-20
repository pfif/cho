use std::collections::HashMap;
use super::amounts::exchange_rates::ExchangeRates;
use super::amounts::{Amount, CurrencyIdent};
use crate::period::{Period, PeriodVaultValues, PeriodsConfiguration};
use chrono::{Local, NaiveDate};
use group::Group;
use rust_decimal_macros::dec;
use crate::accounts::AccountGetter;
use crate::predicted_income::PredictedIncomeBuilder;
use crate::vault::{Vault, VaultReadable};

/* Entrypoint */
pub struct RemainingOperation<P: PeriodsConfiguration> {
    group_factories: Vec<GroupBuilder>,
    periods_configuration: P,
    date: NaiveDate,
}

impl RemainingOperation<PeriodVaultValues> {
    pub fn from_vault_values<V: Vault>(include_predicted_income: bool, vault: &V) -> Result<RemainingOperation<PeriodVaultValues>, String> {
        Ok(
            RemainingOperation{
                date: Local::now().date_naive(),
                periods_configuration: PeriodVaultValues::from_vault(vault)?,
                group_factories: vec![
                    PredictedIncomeBuilder::from_vault_value(include_predicted_income, vault)?.into(),
                    AccountGetter::from_files(vault)?.into()
                ]
            }
        )
    }
}
impl<P: PeriodsConfiguration> RemainingOperation<P> {
    pub fn execute(
        self,
        target_currency: &CurrencyIdent,
        exchange_rates: &ExchangeRates,
    ) -> Result<RemainingOperationScreen, String> {

        let current_period = self
            .periods_configuration
            .period_for_date(&self.date)
            .map_err(|error| "Failed to fetch Periods Configuration: ".to_string() + &error)?;

        let groups = self
            .group_factories
            .into_iter()
            .map(|builder| builder.build(&current_period, &self.date, exchange_rates))
            .collect::<Result<Vec<Group>, String>>()?;

        let mut remaining: Amount = exchange_rates.new_amount(target_currency,dec!(0))?;
        for group in groups.iter() {
            for operand in group.operands() {
                remaining = &remaining + &operand.amount
            }
        }

        Ok(RemainingOperationScreen {
            groups,
            remaining,
        })
    }
}

/* Builders */
pub struct GroupBuilder {
    pub name: String,
    pub operand_factories: Vec<Box<dyn OperandBuilder>>,
}

impl GroupBuilder {
    // TODO Unit tests
    fn build(
        self,
        period: &Period,
        today: &NaiveDate,
        exchange_rates: &ExchangeRates,
    ) -> Result<Group, String> {
        let mut group = group::Group::new(self.name);
        for operand_builder in self.operand_factories.iter() {
            operand_builder
                .build(period, today, exchange_rates)
                .and_then(|operand| group.add_operands(operand))?
        }
        Ok(group)
    }
}

pub trait OperandBuilder {
    fn build(
        &self,
        period: &Period,
        today: &NaiveDate,
        exchange_rates: &ExchangeRates,
    ) -> Result<Operand, String>;
}

/* Output types */
pub struct RemainingOperationScreen {
    pub groups: Vec<Group>,
    pub remaining: Amount,
}

// The struct Group has its own module to isolate its internal attribute
pub mod group {
    use super::Operand;

    /* TODO - we're also not returning the total of each group */
    pub struct Group {
        name: String,
        operands: Vec<Operand>,
        illustration_fields: Option<Vec<String>>,
    }

    impl Group {
        // TODO - Apparently, one shouldn't pass String in? Or so I have heard
        pub(crate) fn new(name: String) -> Group {
            Group {
                name,
                operands: vec![],
                illustration_fields: None,
            }
        }
        // TODO Unit tests
        pub fn add_operands(&mut self, o: Operand) -> Result<(), String> {
            if self.illustration_fields == None {
                let fields = o.illustration.keys().cloned().collect();
                self.illustration_fields = Some(fields)
            } else if let Some(illustration_fields) = &self.illustration_fields {
                if o.illustration.keys().ne(illustration_fields) {
                    return Err(format!(
                        "Adding an operand ({:?}) whose fields ({:?}) does not match that of the rest of the operand in this group ({:?})",
                        o.name,
                        o.illustration.keys(),
                        self.illustration_fields
                    ));
                }

                self.operands.push(o)
            }

            Ok(())
        }

        pub fn operands(&self) -> &Vec<Operand> {
            &self.operands
        }
        pub fn illustration_fields(&self) -> &Option<Vec<String>> {
            &self.illustration_fields
        }
    }
}

// TODO Problem with IlLustrationValue - they exists to split logic from view - we don't dictate
//      how to show the value, we just return a type
//      However, this feels incompatible with not returning a type of Illustration, and instead
//      just a list of columns
//      Downstream code can decide how to display the value, but not only based on its type, not
//      what it _is_
pub enum IllustrationValue {
    Amount(Amount),
    Bool(bool)
}

pub type Illustration = HashMap<String, IllustrationValue>;

pub struct Operand {
    pub name: String,
    pub amount: Amount,
    pub illustration: Illustration,
}
