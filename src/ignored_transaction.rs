use crate::amounts::{CurrencyIdent, Figure};
use crate::period::{AnyPeriodsConfiguration, Period, PeriodsConfiguration};
use crate::remaining::{CalculatorEntry, CalculatorGroupCollector, GroupCombinations};
use crate::vault::{Vault, VaultReadable};
use chrono::NaiveDate;
use serde::Deserialize;

#[cfg_attr(test, derive(Clone))]
#[derive(Deserialize)]
struct IgnoredTransaction {
    name: String,
    currency: CurrencyIdent,
    amount: Figure,
    date: NaiveDate,
}

type IgnoredTransactionsVaultValues = Vec<IgnoredTransaction>;
impl VaultReadable for IgnoredTransactionsVaultValues {
    const KEY: &'static str = "ignored_transactions";
}

/* impl Into<CalculatorEntry> for IgnoredTransaction {
    fn into(self) -> CalculatorEntry {
        CalculatorEntry {
            name: self.name,

            amount_history: currency: self.currency,
            period_start: None,
            current: Some(self.amount),
            predicted_end: None,
        }
    }
}

struct CurrentIgnoredTransactionCollector<'a, V: Vault> {
    vault: &'a V,
}

impl<'a, V: Vault> CalculatorGroupCollector<IgnoredTransaction>
    for CurrentIgnoredTransactionCollector<'a, V>
{
    const GROUP_COMBINATION: &'static GroupCombinations = &GroupCombinations::SUBTRACT;

    fn collect_raw<P: PeriodsConfiguration>(
        &self,
        period_config: &P,
        date: &NaiveDate,
    ) -> Result<impl Iterator<Item = IgnoredTransaction>, String> {
        let current_period = period_config.period_for_date(date)?;
        let ignored_transactions = IgnoredTransactionsVaultValues::from_vault(self.vault)?;
        Ok(ignored_transactions
            .into_iter()
            .filter(move |ignored_transaction| {
                ignored_transaction.date >= current_period.start_date
                    && ignored_transaction.date <= *date
            }))
    }
} */

/* #[cfg(test)]
mod test_calculatorgroup_collector {
    use std::ptr::null;
    use crate::ignored_transaction::{CurrentIgnoredTransactionCollector, IgnoredTransaction};
    use crate::period::Period;
    use crate::vault::Vault;
    use chrono::NaiveDate;
    use derive_builder::Builder;

    #[derive(Builder)]
    #[builder(pattern = "immutable", build_fn(skip), name = "Test")]
    struct Tes {
        vault_values: Vec<IgnoredTransaction>,

        current_period: Period,
        current_date: NaiveDate,

        expected: Vec<IgnoredTransaction>,
    }

    impl Test {
        fn execute<V: Vault>(self) {
            let instance = CurrentIgnoredTransactionCollector{
                vault: null(),
            };
            let result = CurrentIgnoredTransactionCollector::filter_to_period(
                self.vault_values.unwrap(),
                self.current_period.unwrap(),
                self.current_date.unwrap()
            );
        }
    }
}
*/
