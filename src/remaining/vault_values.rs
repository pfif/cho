use serde::Deserialize;

use crate::period::AnyPeriodsConfiguration;
use crate::vault::VaultReadable;

#[derive(Deserialize)]
pub struct RemainingVaultValues {
    pub periods_configuration: AnyPeriodsConfiguration,
}

impl VaultReadable for RemainingVaultValues {
    const KEY: &'static str = "remaining";
}
