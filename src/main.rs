use crate::cli::remaining_operation;

mod accounts;
mod amounts;
mod cli;
mod goals;
mod ignored_transaction;
mod period;
mod predicted_transaction;
mod remaining;
mod vault;

fn main() {
    remaining_operation()
}
