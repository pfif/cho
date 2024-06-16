pub use periods::Period;
pub use periods_configuration::{AnyPeriodsConfiguration, PeriodsConfiguration};
#[cfg(test)]
pub use periods_configuration::MockPeriodsConfiguration;

mod calendar_month_period;
mod fixed_length_period;
mod periods;
mod periods_configuration;

