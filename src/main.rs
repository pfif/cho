mod accounts;
mod cli;
mod ignored_transaction;
mod period;
mod vault;
mod remaining_operation;
mod predicted_income;
mod buckets;
pub mod amounts;
pub mod chrono_stack;
mod predicted_transaction;
mod line;

use crate::cli::remaining_operation;
fn main() {
    remaining_operation()
}
