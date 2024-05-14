mod interface;
mod fixed_length_period;

pub use interface::{PeriodsConfiguration, Period};

#[cfg(test)]
pub use interface::{MockPeriodsConfiguration};
pub use fixed_length_period::{PeriodVaultValues};
