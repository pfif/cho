use std::fmt::{Debug, Formatter};
use std::ops::{Add, Sub};
use crate::amounts::{Amount};
use crate::amounts::exchange_rates::ExchangeRates;
use crate::buckets::Action;

#[derive(Clone)]
pub struct AggregatedAmounts {
    total: Amount,
    deposited: Amount,
    withdrawn: Amount,

    exchange_rates: ExchangeRates,
}

impl AggregatedAmounts {
    pub fn new(exchange_rates: &ExchangeRates) -> Result<Self, String> {
        // TODO - TERRIBLE HACK AROUND THE FACT THAT I NEED TO CHOOSE A CURRENCY FOR AMOUNT
        //        IN THE FUTURE I WILL MAKE AMOUNT WITHOUT CURRENCIES (currencies are removed when the
        //        RawAmount is converted to Amount, and added back when exited from the system)
        let zero_yen = exchange_rates.zero(&"JPY".to_string())?;
        Ok(AggregatedAmounts {
            total: zero_yen.clone(),
            withdrawn: zero_yen.clone(),
            deposited: zero_yen.clone(),

            exchange_rates: exchange_rates.clone(),
        })
    }

    pub fn apply(&mut self, action: &Action) -> Result<(), String> {
        debug_assert!(self.total == self.deposited.clone() - self.withdrawn.clone(), "Total is always equal to deposited - withdrawn.");
        let amount = match action {
            | Action::Deposit(amount)
            | Action::DepositCancellation(amount)
            | Action::Withdrawal(amount)
            | Action::WithdrawalCancellation(amount) => {
                self.exchange_rates.new_amount_from_raw_amount(amount)?
            },
            _ => return Ok(())
        };

        match action {
            Action::Deposit(_) => {
                self.deposited = self.deposited.clone() + amount.clone();
            }
            Action::DepositCancellation(_) => {
                let new_deposited = self.deposited.clone() - amount.clone();
                if new_deposited.is_negative() {
                    return Err("attempt to remove more than was deposited".to_string());
                }
                self.deposited = new_deposited;
            },
            Action::Withdrawal(_) => {
                self.withdrawn = self.withdrawn.clone() + amount.clone();
            },
            Action::WithdrawalCancellation(_) => {
                let new_withdrawn = self.withdrawn.clone() - amount.clone();
                if new_withdrawn.is_negative() {
                    return Err("attempt to put back money that was not withdrawn".to_string());
                }
                self.withdrawn = new_withdrawn;
            }
            _ => {}
        };

        match action {
            Action::Deposit(_) | Action::WithdrawalCancellation(_) => {
                self.total = self.total.clone() + amount.clone();
            },
            Action::Withdrawal(_) | Action::DepositCancellation(_) => {
                self.total = self.total.clone() - amount;
            },
            _ => {}
        }
        Ok(())
    }

    pub fn total(&self) -> Amount { self.total.clone() }
    pub fn deposited(&self) -> Amount { self.deposited.clone() }
    pub fn withdrawn(&self) -> Amount { self.withdrawn.clone() }
}

impl Debug for AggregatedAmounts {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "AggregatedAmounts {{ total: {:?}, deposited: {:?}, withdrawn: {:?} }}", self.total, self.deposited, self.withdrawn)
    }
}

impl Sub<AggregatedAmounts> for AggregatedAmounts {
    type Output = AggregatedAmounts;

    fn sub(self, rhs: AggregatedAmounts) -> Self::Output {
        AggregatedAmounts{
            total: self.total - rhs.total,
            deposited: self.deposited - rhs.deposited,
            withdrawn: self.withdrawn - rhs.withdrawn,
            exchange_rates: self.exchange_rates,
        }
    }
}


mod tests {
    use chrono::Days;
    use crate::amounts::{RawAmount};
    use super::*;

    #[test]
    fn totals() {
        let ex = ExchangeRates::for_tests();

        let base_state = AggregatedAmounts {
            total: ex.yen("100"),
            deposited: ex.yen("500"),
            withdrawn: ex.yen("400"),

            exchange_rates: ex.clone(),
        };

        struct TestTable {
            name: String,
            action: Action,

            expected_result: ExpectedResult,
        }
        ;

        enum ExpectedResult {
            Success {
                expected_total: Amount,
                expected_deposited: Amount,
                expected_withdrawn: Amount,
            },
            Failure {
                error: String,
            },
        }

        let tests = vec![
            TestTable {
                name: "Deposit".to_string(),
                action: Action::Deposit(RawAmount::yen("5")),

                expected_result: ExpectedResult::Success {
                    expected_total: base_state.total.clone() + ex.yen("5"),
                    expected_deposited: base_state.deposited.clone() + ex.yen("5"),
                    expected_withdrawn: base_state.withdrawn.clone()
                }
            },
            TestTable {
                name: "DepositCancellation - small".to_string(),
                action: Action::DepositCancellation(RawAmount::yen("5")),

                expected_result: ExpectedResult::Success {
                    expected_total: base_state.total.clone() - ex.yen("5"),
                    expected_deposited: base_state.clone().deposited - ex.yen("5"),
                    expected_withdrawn: base_state.withdrawn.clone()
                }
            },
            TestTable {
                name: "DepositCancellation - cancels everything".to_string(),
                action: Action::DepositCancellation(RawAmount::from(base_state.deposited.clone())),

                expected_result: ExpectedResult::Success {
                    expected_total: base_state.total.clone() - base_state.deposited.clone(),
                    expected_deposited: ex.yen("0"),
                    expected_withdrawn: base_state.withdrawn.clone()
                }
            },
            TestTable {
                name: "DepositCancellation - cancels more than exists".to_string(),
                action: Action::DepositCancellation(
                    RawAmount::from(
                        base_state.deposited.clone() + ex.yen("100" )
                    )
                ),

                expected_result: ExpectedResult::Failure { error: "attempt to remove more than was deposited".to_string() }
            },
            TestTable {
                name: "Withdraw".to_string(),
                action: Action::Withdrawal(RawAmount::yen("5")
                ),

                expected_result: ExpectedResult::Success {
                    expected_total: base_state.total.clone() - ex.yen("5"),
                    expected_deposited: base_state.deposited.clone(),
                    expected_withdrawn: base_state.withdrawn.clone() + ex.yen("5")
                }
            },
            TestTable {
                name: "Withdraw remaining".to_string(),
                action: Action::Withdrawal(
                    RawAmount::from(
                        // Withdraw what has been deposited but not yet withdrawn, which is the
                        // total
                        base_state.total.clone())
                ),

                expected_result: ExpectedResult::Success {
                    expected_total: ex.yen("0"),
                    expected_deposited: base_state.deposited.clone(),
                    expected_withdrawn: base_state.deposited.clone(),
                }
            },
            TestTable {
                name: "Withdraw more than was deposited".to_string(),
                action: Action::Withdrawal(
                    RawAmount::from(
                        // Withdraw the remaining amount that has not yet been withdrawn (total)
                        // and some more (100 yens)
                        base_state.total.clone() + ex.yen("100"))
                ),

                expected_result: ExpectedResult::Success {
                    expected_total: ex.yen("-100"),
                    expected_deposited: base_state.deposited.clone(),
                    expected_withdrawn: base_state.withdrawn.clone() + base_state.total.clone() + ex.yen("100"),
                }
            },
            TestTable {
                name: "Withdraw cancellation".to_string(),
                action: Action::WithdrawalCancellation(
                    RawAmount::yen("5")

                ),

                expected_result: ExpectedResult::Success {
                    expected_total: base_state.total.clone() + ex.yen("5"),
                    expected_deposited: base_state.deposited.clone(),
                    expected_withdrawn: base_state.withdrawn.clone() - ex.yen("5"),
                }
            },
            TestTable {
                name: "Withdraw cancellation - cancels everything".to_string(),
                action: Action::WithdrawalCancellation(
                    RawAmount::from(base_state.withdrawn.clone())

                ),

                expected_result: ExpectedResult::Success {
                    expected_total: base_state.total.clone() + base_state.withdrawn.clone(),
                    expected_deposited: base_state.deposited.clone(),
                    expected_withdrawn: ex.yen("0"),
                }
            },
            TestTable {
                name: "Withdraw cancellation - cancels too much".to_string(),
                action: Action::WithdrawalCancellation(
                    RawAmount::from(base_state.withdrawn.clone() + ex.yen("100"))

                ),

                expected_result: ExpectedResult::Failure {
                    error: "attempt to put back money that was not withdrawn".to_string()
                }
            }
        ];

        for test in tests {
            let mut state = base_state.clone();
            let result = state.apply(
                &test.action
            );

            match test.expected_result {
                ExpectedResult::Success { expected_total, expected_deposited, expected_withdrawn } => {
                    result.expect("did succeed");

                    assert_eq!(state.total, expected_total, "{}", test.name);
                    assert_eq!(state.deposited, expected_deposited, "{}", test.name);
                    assert_eq!(state.withdrawn, expected_withdrawn, "{}", test.name);
                },
                ExpectedResult::Failure { error } => {
                    assert_eq!(result, Err(error))
                }
            }
        }
    }

    #[test]
    fn subtraction() {
        todo!()
    }
}