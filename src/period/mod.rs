pub use periods::Period;
#[cfg(test)]
pub use periods_configuration::MockPeriodsConfiguration;
pub use periods_configuration::{AnyPeriodsConfiguration, PeriodsConfiguration};

mod calendar_month_period;
mod fixed_length_period;
mod periods;
mod periods_configuration;
