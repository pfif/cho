#[cfg(test)]
mod format_remaining_operation_screen_tests {
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;
    use crate::cli::formatting::format_remaining_operation_screen;
    use crate::period::Period;
    use pretty_assertions::assert_eq;
    use crate::amounts::exchange_rates::ExchangeRates;
    use crate::remaining_operation::core_types::group::Group;
    use crate::remaining_operation::core_types::{Illustration, IllustrationValue, Operand, RemainingOperationScreen, RemainingOperationScreenGroup, RemainingOperationScreenOperand};
    const ILLUSTRATION_FIELDS : &[&str] = &["First amount", "Second amount", "Is enough", "Is luxury"];
    const ILLUSTRATION_FIELDS_WITH_EXTRA : &[&str] = &["First amount", "Second amount", "Is enough", "Is luxury", "Extra column"];
    
    fn make_operand(exchange_rates: &ExchangeRates, name: String, extra_column: bool, null_amount: bool) -> RemainingOperationScreenOperand {
        let five = exchange_rates.new_amount(&"JPY".to_string(), dec!(5)).expect("Could create amount");
        let six = exchange_rates.new_amount(&"EUR".to_string(), dec!(6)).expect("Could create amount");
        
        let mut illustration: Illustration = Vec::new();
        illustration.push(("First amount".into(), IllustrationValue::Amount(five.clone())));
        illustration.push(("Second amount".into(), if !null_amount {IllustrationValue::Amount(six.clone())} else {IllustrationValue::NullAmount}));
        illustration.push(("Is enough".into(), IllustrationValue::Bool(true)));
        illustration.push(("Is luxury".into(), IllustrationValue::Bool(false)));
        if extra_column {
           illustration.push(("Extra column".into(), IllustrationValue::Bool(true))); 
        }

        RemainingOperationScreenOperand{
            name,
            amount: five.clone(),
            illustration,
        }
    }

    #[test]
    fn test() {
        let date = NaiveDate::from_ymd_opt(2026,5, 9).expect("Can create date");
        let exchange_rates = ExchangeRates::for_tests();

        let empty_group = RemainingOperationScreenGroup {
            name: "Empty".to_string(),
            operands: vec![],
            illustration_fields: vec![],
            total: exchange_rates.zero(&"JPY".to_string()).expect("Could create amount"),
            archived_operand_with_non_zero_amounts: vec![],
        };

        let empty_group_with_wrongly_archived = RemainingOperationScreenGroup {
            name: "Empty with wrongly archived".to_string(),
            operands: vec![],
            illustration_fields: vec![],
            total: exchange_rates.zero(&"JPY".to_string()).expect("Could create amount"),
            archived_operand_with_non_zero_amounts: vec![
                "LINE".to_string(),
                "Credit Mutuel".to_string()
            ],
        };

        let normal_group = RemainingOperationScreenGroup{
            name: "Normal group".to_string(),
            total: exchange_rates.new_amount(&"EUR".to_string(), dec!(7.50)).expect("Could create amount"),
            operands: vec![
                make_operand(&exchange_rates, "Payment for house".into(), false, false),
                make_operand(&exchange_rates, "Payment for dog".into(), false, false),
                make_operand(&exchange_rates, "Payment for cat".into(), false, true)
            ],
            illustration_fields: ILLUSTRATION_FIELDS.iter().map(|field| field.to_string()).collect(),
            archived_operand_with_non_zero_amounts: vec!["Payment for penny-farthing".to_string()]
        };
        
        let extra_column_group = RemainingOperationScreenGroup{
            name: "Extra column group".to_string(),
            total: exchange_rates.new_amount(&"EUR".to_string(), dec!(7.50)).expect("Could create amount"),
            operands: vec![
                make_operand(&exchange_rates, "Payment for Mr Spock".into(), true, false),
                make_operand(&exchange_rates, "Payment for Jean Luc".into(), true, false),
                make_operand(&exchange_rates, "Payment for Katherine".into(), true, false)
            ],
            illustration_fields: ILLUSTRATION_FIELDS_WITH_EXTRA.iter().map(|field| field.to_string()).collect(),
            archived_operand_with_non_zero_amounts: vec![]
        };


        let groups = vec![empty_group, empty_group_with_wrongly_archived, normal_group, extra_column_group];
        let screen = RemainingOperationScreen{
            groups,
            remaining: exchange_rates.new_amount(&"EUR".to_string(), dec!(100)).expect("Could create amount"),
            period: Period{
                start_date: NaiveDate::from_ymd_opt(2025,1,1).unwrap(),
                end_date: NaiveDate::from_ymd_opt(2025, 1, 31).unwrap()
            },
        };
        
        // TODO add a test to see the formatting of a group with an archived operand that should not be archived
        assert_eq!(format_remaining_operation_screen(&screen), r#"Current period : 2025-01-01 to 2025-01-31
=========================================

Empty
=====
No operands for this period

Empty with wrongly archived
===========================
⚠ LINE and Credit Mutuel have been archived, but their current amount is not zero and still impact the group's total.

Normal group
============
+-------------------+--------+--------------+---------------+-----------+-----------+
| Name              | Amount | First amount | Second amount | Is enough | Is luxury |
+===================================================================================+
| Payment for house | ¥5     | ¥5           | €6            | ✅        |           |
|-------------------+--------+--------------+---------------+-----------+-----------|
| Payment for dog   | ¥5     | ¥5           | €6            | ✅        |           |
|-------------------+--------+--------------+---------------+-----------+-----------|
| Payment for cat   | ¥5     | ¥5           | -             | ✅        |           |
|-------------------+--------+--------------+---------------+-----------+-----------|
| Total             | €7.50  |              |               |           |           |
+-------------------+--------+--------------+---------------+-----------+-----------+
⚠ Payment for penny-farthing has been archived, but its current amount is not zero and still impact the group's total.

Extra column group
==================
+-----------------------+--------+--------------+---------------+-----------+-----------+--------------+
| Name                  | Amount | First amount | Second amount | Is enough | Is luxury | Extra column |
+======================================================================================================+
| Payment for Mr Spock  | ¥5     | ¥5           | €6            | ✅        |           | ✅           |
|-----------------------+--------+--------------+---------------+-----------+-----------+--------------|
| Payment for Jean Luc  | ¥5     | ¥5           | €6            | ✅        |           | ✅           |
|-----------------------+--------+--------------+---------------+-----------+-----------+--------------|
| Payment for Katherine | ¥5     | ¥5           | €6            | ✅        |           | ✅           |
|-----------------------+--------+--------------+---------------+-----------+-----------+--------------|
| Total                 | €7.50  |              |               |           |           |              |
+-----------------------+--------+--------------+---------------+-----------+-----------+--------------+

Remaining this period: €100
=============================

Release: Development build
"#.to_string())
    }
}