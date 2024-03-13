/* use crate::goals::get_goals;
use crate::remaining::remaining_money;
use crate::vault::get_vault_value;
use chrono::NaiveDate;


/*
Period start: 2023/02/01
============

Accounts
========
             | ING       | Credit Mutuel     | Wise            | LINE            | ゆうちょ            | Liquide      | Total
Period start | €0        | €54775.19         | €2889.14        | ¥24796          | ¥3758343            | ¥13000       | €81518.52
Current      | €0 (-€0)  | €54000.00 (-€547) | €3000 (+€28)    | ¥22300 (-¥796)  | ¥3266780 (-¥491563) | ¥12987 (-¥13)| €79518.52 (-€2000)

(+) Predicted Income: €2000
====================

(-) Goals
=========
               | Commited | Commited this period | Target |
Retirment fund | €15000   | … (€500)             | €60000 |
New iPhone     | €300     | ✅                   | €1000  |
Total          | €15300   | €2000 (€2500)        | €61000 |

Remaining this period: €456
*/

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
