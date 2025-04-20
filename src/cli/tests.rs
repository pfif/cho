#[cfg(test)]
mod format_remaining_operation_screen_tests {
    use std::collections::HashMap;
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;
    use crate::cli::formatting::format_remaining_operation_screen;
    use crate::period::Period;
    use crate::remaining_operation::amounts::{Amount, Currency};
    use crate::remaining_operation::core_types::group::Group;
    use crate::remaining_operation::core_types::{Illustration, IllustrationValue, Operand, RemainingOperationScreen};
    
    fn make_currency(sign: &str) -> Currency {
        Currency {
            rate: dec!(1),
            sign: sign.into()
        }
    }
    
    fn make_amount(sign: &str, value: rust_decimal::Decimal) -> Amount {
        let currency = make_currency(sign);
        Amount::new_mock(&currency, value)
    }
    
    fn make_operand(name: String, extra_column: bool) -> Operand {
        let five = make_amount("CREDIT", dec!(5));
        let six = make_amount("EURO", dec!(6));
        
        let mut illustration: Illustration = Vec::new();
        illustration.push(("First amount".into(), IllustrationValue::Amount(five.clone())));
        illustration.push(("Second amount".into(), IllustrationValue::Amount(six.clone())));
        illustration.push(("Is enough".into(), IllustrationValue::Bool(true)));
        illustration.push(("Is luxury".into(), IllustrationValue::Bool(false)));
        if extra_column {
           illustration.push(("Extra column".into(), IllustrationValue::Bool(true))); 
        }
        
        Operand{
            name,
            amount: five.clone(),
            illustration,
            
        }
    }

    struct TestTable {
        include_empty_group: bool,
        include_normal_group: bool,
        include_extra_column_group: bool,
        
        expected_output: String
    }
    impl TestTable {
        fn test(self) {
            let mut groups = vec![];
            if self.include_empty_group {
                let empty_group = Group::new("Empty".into());
                groups.push(empty_group);
            }
             
            if self.include_normal_group {
                let mut normal_group = Group::new("Normal group".into());
                normal_group.add_operands(make_operand("Payment for house".into(), false));
                normal_group.add_operands(make_operand("Payment for dog".into(), false));
                normal_group.add_operands(make_operand("Payment for cat".into(), false));
                
                groups.push(normal_group);
            }
            
            if self.include_extra_column_group {
                let mut extra_column_group = Group::new("Extra column group".into());
                extra_column_group.add_operands(make_operand("Payment for Mr Spock".into(), true));
                extra_column_group.add_operands(make_operand("Payment for Jean Luc".into(), true));
                extra_column_group.add_operands(make_operand("Payment for Katherine".into(), true));
                
                groups.push(extra_column_group);
            }
            
            let screen = RemainingOperationScreen{
                groups,
                remaining: make_amount("EURO", dec!(100)),
                period: Period{
                    start_date: NaiveDate::from_ymd_opt(2025,1,1).unwrap(),
                    end_date: NaiveDate::from_ymd_opt(2025, 1, 31).unwrap()
                },
            };
            
            assert_eq!(format_remaining_operation_screen(&screen), self.expected_output)
        }
    }
    
    #[test]
    fn test_all_groups(){
        TestTable{
            include_empty_group: true,
            include_normal_group: true,
            include_extra_column_group: true,
            expected_output: r#"Current period : 2025-01-01 to 2025-01-31
=========================================

Empty
=====
+------+
| Name |
+======+
+------+

Normal group
============
+-----------------+--------------+---------------+-----------+-----------+
| Name            | First amount | Second amount | Is enough | Is luxury |
+========================================================================+
| Payment for dog | CREDIT5      | EURO6         | ✅        |           |
|-----------------+--------------+---------------+-----------+-----------|
| Payment for cat | CREDIT5      | EURO6         | ✅        |           |
+-----------------+--------------+---------------+-----------+-----------+

Extra column group
==================
+-----------------------+--------------+---------------+-----------+-----------+--------------+
| Name                  | First amount | Second amount | Is enough | Is luxury | Extra column |
+=============================================================================================+
| Payment for Jean Luc  | CREDIT5      | EURO6         | ✅        |           | ✅           |
|-----------------------+--------------+---------------+-----------+-----------+--------------|
| Payment for Katherine | CREDIT5      | EURO6         | ✅        |           | ✅           |
+-----------------------+--------------+---------------+-----------+-----------+--------------+

Remaining this period: EURO100
=============================="#.to_string(),
        }.test()
    }
}