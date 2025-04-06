mod accounts;
mod cli;
mod goals;
mod ignored_transaction;
mod period;
mod remaining;
mod vault;
mod remaining_operation;

use crate::cli::remaining_operation;
fn main() {
    remaining_operation()
}
