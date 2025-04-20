mod accounts;
mod cli;
mod goals;
mod ignored_transaction;
mod period;
mod vault;
mod remaining_operation;
mod predicted_income;

use crate::cli::remaining_operation;
fn main() {
    remaining_operation()
}
