use chrono::{Local, NaiveDate};

use crate::accounts::{get_accounts, AccountJson, QueriableAccount};
// use crate::amounts::ExchangeRates;
use crate::goals::{Goal, GoalImplementation, GoalVaultValues};
use crate::period::{AnyPeriodsConfiguration, PeriodsConfiguration};
use crate::remaining::legacy::{compute_legacy_remaining_screen, RemainingMoneyScreen, ExchangeRates};
use crate::remaining::{
    legacy::{Amount, Currency},
    vault_values::RemainingVaultValues,
};
use crate::vault::{Vault, VaultReadable};

pub struct RemainingOperation<A: QueriableAccount, G: Goal<P>, P: PeriodsConfiguration> {
    pub rates: ExchangeRates,
    pub target_currency: Currency,

    pub date: NaiveDate,
    pub periods_configuration: P,

    pub raw_accounts: Vec<A>,
    pub goals: Vec<G>,

    pub predicted_income: Option<Amount>,
}

impl RemainingOperation<AccountJson, GoalImplementation, AnyPeriodsConfiguration> {
    pub fn from_vault_value<V: Vault>(
        exchange_rate: ExchangeRates,
        target_currency: Currency,
        predicted_income: Option<Amount>,
        vault: &V,
    ) -> Result<RemainingOperation<AccountJson, GoalImplementation, AnyPeriodsConfiguration>, String>
    {
        return Ok(RemainingOperation {
            rates: exchange_rate,
            target_currency,

            date: Local::now().date_naive(),
            periods_configuration: RemainingVaultValues::from_vault(vault)?.periods_configuration,

            raw_accounts: get_accounts(vault)?,
            goals: GoalVaultValues::from_vault(vault)?,

            predicted_income,
        });
    }
}

impl<A: QueriableAccount, G: Goal<P>, P: PeriodsConfiguration> RemainingOperation<A, G, P> {
    pub fn execute(self) -> Result<RemainingOperationOutput, String> {
        Ok(RemainingOperationOutput {
            legacy_money_screen: compute_legacy_remaining_screen(&self)?,
        })
    }
}

pub struct RemainingOperationOutput {
    pub legacy_money_screen: RemainingMoneyScreen,
}
