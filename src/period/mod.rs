mod periods_configuration;
mod fixed_length_period;
mod calendar_month_period;
mod periods;

pub use periods_configuration::{AnyPeriodsConfiguration, PeriodsConfiguration};
pub use periods::Period;

#[cfg(test)]
pub use periods_configuration::MockPeriodsConfiguration;