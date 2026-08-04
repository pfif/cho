use crate::amounts::Amount;
use crate::period::{Period, PeriodsConfiguration};
use crate::remaining_operation::core_types::Operand;

#[derive(Debug, PartialEq, Eq)]
pub struct PredictedTransaction {
    name: String,
    amount: Amount,
}

impl PredictedTransaction {
    pub(super) fn new<P: PeriodsConfiguration>(
        template_name: String,
        period: Period,
        amount: Amount,
    ) -> Result<PredictedTransaction, String> {
        Ok(PredictedTransaction {
            name: format!("{} - {}", template_name, P::id_for_period(&period)?),
            amount,
        })
    }

    pub(super) fn build_operand(self) -> Operand {
        Operand {
            name: self.name,
            amount: self.amount.flip_sign(),
            illustration: vec![],
            archived_from: None,
        }
    }
}