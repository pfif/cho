use crate::cli::remaining_operation;

mod accounts;
mod amounts;
mod cli;
mod goals;
mod period;
mod remaining;
mod transaction;
mod vault;

fn main() {
    remaining_operation()
}
