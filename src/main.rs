mod accounts;
mod cli;
mod goals;
mod period;
mod remaining;
mod vault;
mod transaction;

use crate::cli::remaining_operation;
fn main() {
    remaining_operation()
}
