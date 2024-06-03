mod interface;
mod fixed_length_period;
mod calendar_month_period;

pub use interface::{PeriodsConfiguration, Period, AnyPeriodsConfiguration};

#[cfg(test)]
pub use interface::{MockPeriodsConfiguration};