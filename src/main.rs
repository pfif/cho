mod accounts;
mod amounts;
mod cli;
mod goals;
mod period;
mod remaining;
mod transaction;
mod vault;

use crate::cli::remaining_operation;
fn main() {
    remaining_operation()
}
