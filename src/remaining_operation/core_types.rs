use std::collections::HashMap;
use super::amounts::exchange_rates::ExchangeRates;
use super::amounts::{Amount, CurrencyIdent};
use crate::period::{Period, PeriodConfigurationVaultValue, PeriodsConfiguration};
use crate::goals::{GoalVaultValues};
use chrono::{Local, NaiveDate};
use group::Group;
use rust_decimal_macros::dec;
use crate::accounts::AccountGetter;
use crate::predicted_income::{PredictedIncome};
use crate::vault::{Vault, VaultReadable};

/* Entrypoint */
pub struct RemainingOperation {
    group_factories: Vec<GroupBuilder>,
    periods_configuration: PeriodConfigurationVaultValue,
    date: NaiveDate,
}

impl RemainingOperation {
    pub fn from_vault_values<V: Vault>(include_predicted_income: bool, vault: &V) -> Result<RemainingOperation, String> {
        let mut group_factories: Vec<GroupBuilder> = vec![
            AccountGetter::from_files(vault)?.into(),
            GoalVaultValues::from_vault(vault)?.into() 
        ];
        if include_predicted_income {
            group_factories.push(PredictedIncome::from_vault(vault)?.into());
        }
        Ok(
            RemainingOperation{
                date: Local::now().date_naive(),
                periods_configuration: PeriodConfigurationVaultValue::from_vault(vault)?,
                group_factories
            }
        )
    }
}
impl RemainingOperation {
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
            .map(|builder| builder.build(&self.periods_configuration, &self.date, exchange_rates))
            .collect::<Result<Vec<Group>, String>>()?;

        let mut remaining: Amount = exchange_rates.new_amount(target_currency,dec!(0))?;
        for group in groups.iter() {
            for operand in group.operands() {
                remaining = &remaining + &operand.amount
            }
        }

        Ok(RemainingOperationScreen {
            period: current_period,
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
        period_configuration: &PeriodConfigurationVaultValue,
        today: &NaiveDate,
        exchange_rates: &ExchangeRates,
    ) -> Result<Group, String> {
        let mut group = group::Group::new(self.name);
        for operand_builder in self.operand_factories.iter() {
            operand_builder
                .build(period_configuration, today, exchange_rates)
                .and_then(|operand| {
                    match operand {
                        Some(operand) => group.add_operands(operand),
                        None => Ok(())
                    }
                })?
        }
        Ok(group)
    }
}

pub trait OperandBuilder {
    // TODO shouldn't this consume self?
    fn build(
        &self,
        period_configuration: &PeriodConfigurationVaultValue,
        today: &NaiveDate,
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
    }
}

// TODO Problem with IlLustrationValue - they exists to split logic from view - we don't dictate
//      how to show the value, we just return a type
//      However, this feels incompatible with not returning a type of Illustration, and instead
//      just a list of columns
//      Downstream code can decide how to display the value, but not only based on its type, not
//      what it _is_
#[derive(Clone, Debug)]
pub enum IllustrationValue {
    Amount(Amount),
    Bool(bool)
}

pub type Illustration = Vec<(String, IllustrationValue)>;

#[derive(Debug)]
pub struct Operand {
    pub name: String,
    pub amount: Amount,
    pub illustration: Illustration,
}
