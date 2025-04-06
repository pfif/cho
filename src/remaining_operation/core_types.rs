use std::collections::HashMap;
use super::amounts::exchange_rates::ExchangeRates;
use super::amounts::{Amount, CurrencyIdent};
use crate::period::Period;
use chrono::NaiveDate;
use group::Group;
use rust_decimal_macros::dec;

/* Entrypoint */
struct RemainingOperation {
    group_factories: Vec<GroupBuilder>,
}

impl RemainingOperation {
    fn with_live_data() -> RemainingOperation {
        todo!()
    }
    fn execute(
        self,
        period: &Period,
        today: &NaiveDate,

        target_currency: &CurrencyIdent,
        exchange_rates: &ExchangeRates,
    ) -> Result<RemainingOperationScreen, String> {
        let groups = self
            .group_factories
            .iter()
            .map(|builder| builder.build(period, today, exchange_rates))
            .collect::<Result<Vec<Group>, String>>()?;

        let mut remaining: Amount = exchange_rates.new_amount(target_currency,dec!(0))?;
        for group in groups.iter() {
            for operand in group.operands() {
                remaining = &remaining + &operand.amount
            }
        }

        Ok(RemainingOperationScreen {
            groups: groups,
            remaining: remaining,
        })
    }
}

/* Builders */
struct GroupBuilder {
    operand_factories: Vec<Box<dyn OperandBuilder>>,
}

impl GroupBuilder {
    // TODO Unit tests
    fn build(
        &self,
        period: &Period,
        today: &NaiveDate,
        exchange_rates: &ExchangeRates,
    ) -> Result<Group, String> {
        let mut group = group::Group::new();
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
struct RemainingOperationScreen {
    pub groups: Vec<Group>,
    pub remaining: Amount,
}

// The struct Group has its own module to isolate its internal attribute
mod group {
    use super::Operand;

    pub struct Group {
        operands: Vec<Operand>,
        illustration_fields: Option<Vec<String>>,
    }

    impl Group {
        pub(crate) fn new() -> Group {
            Group {
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

pub struct Operand {
    name: String,
    amount: Amount,
    illustration: HashMap<String, String>,
}
