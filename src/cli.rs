/* use crate::goals::get_goals;
use crate::remaining::remaining_money;
use crate::vault::get_vault_value;
use chrono::NaiveDate;

fn remaining() {
    // TODO Parse arguments
    let exchange_rate: ((String, f64), (String, f64));
    let target_currency: String;
    let include_predicted_income: bool;

    let vault_value = get_vault_value();

    let remaining_money = remaining_money(
        exchange_rate,
        target_currency,
        vault_value,
        vault_value,
        include_predicted_income,
    );
    // TODO: Display remaining money
}

fn goals() {
    // TODO Parse arguments
    let date: NaiveDate;
    let vault_value = get_vault_value();

    get_goals(vault_value, date);

    // TODO Display goals
}
*/
