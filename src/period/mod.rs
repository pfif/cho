mod calendar_month_period;
mod fixed_length_period;
mod interface;

pub use interface::{Period, PeriodConfigurationVaultValue, PeriodsConfiguration, ErrorPeriodsBetween};
pub use calendar_month_period::CalendarMonthPeriodConfiguration;

#[cfg(test)]
pub use interface::MockPeriodsConfiguration;
