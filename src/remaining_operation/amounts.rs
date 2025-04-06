use crate::remaining_operation::amounts::amount::ImmutableAmount;
use rust_decimal::Decimal;
use std::ops;

pub type Figure = Decimal;
pub type CurrencyIdent = String;
pub type Sign = String;

// TODO To make this code more efficient, to make sure we keep only one version of the currency in memory and as an exercice for me to understand lifetimes,
//      remove the clone from here and use lifetimes.
//      We shouldn't need more than one instance per currency
#[derive(Clone)]
pub struct Currency {
    pub rate: Figure,
    pub sign: String,
}

pub mod exchange_rates {
    use super::{Amount, Currency, CurrencyIdent, Figure};
    use crate::remaining_operation::amounts::amount::ImmutableAmount;
    use std::collections::HashMap;

    pub struct ExchangeRates {
        rate: HashMap<CurrencyIdent, Currency>,
    }

    impl ExchangeRates {
        pub fn get_currency(&self, ident: &CurrencyIdent) -> Result<&Currency, String> {
            self.rate
                .get(ident)
                .ok_or(format!("Could not find currency ident: {}", ident))
        }

        pub fn new_amount(
            &self,
            currency_ident: &CurrencyIdent,
            figure: Figure,
        ) -> Result<Amount, String> {
            let currency = self.get_currency(currency_ident)?;
            Ok(Amount {
                immutable_amount: ImmutableAmount::new(currency, figure),
            })
        }
    }
}

mod amount {
    use crate::remaining_operation::amounts::{Currency, Figure};

    #[derive(Clone)]
    pub struct ImmutableAmount {
        currency: Currency,
        figure: Figure,
    }

    impl ImmutableAmount {
        pub fn new(currency: &Currency, figure: Figure) -> Self {
            Self {
                currency: currency.clone(),
                figure: figure
                    .round_dp_with_strategy(2, rust_decimal::RoundingStrategy::MidpointNearestEven),
            }
        }

        pub fn currency(&self) -> &Currency {
            &self.currency
        }
        pub fn figure(&self) -> &Figure {
            &self.figure
        }
    }
}

// Amount should be instantiated using the ExchangeRate object
#[derive(Clone)]
pub struct Amount {
    immutable_amount: ImmutableAmount,
}

impl Amount {
    fn convert(&self, target_currency: &Currency) -> Amount {
        let exchange_rate = {
            let target_currency_value = target_currency.rate;
            let from_currency_value = self.immutable_amount.currency().rate;
            target_currency_value / from_currency_value
        };

        let new_immutable_amount = ImmutableAmount::new(
            target_currency,
            self.immutable_amount.figure() * exchange_rate,
        );

        Amount {
            immutable_amount: new_immutable_amount,
        }
    }
}

impl ops::Add for &Amount {
    type Output = Amount;

    fn add(self, other_amount: Self) -> Self::Output {
        let other_amount_converted = other_amount.convert(self.immutable_amount.currency());
        let new_immutable_amount = ImmutableAmount::new(
            self.immutable_amount.currency(),
            self.immutable_amount.figure() + other_amount_converted.immutable_amount.figure(),
        );
        Amount {
            immutable_amount: new_immutable_amount,
        }
    }
}

impl ops::Sub for &Amount {
    type Output = Amount;

    fn sub(self, other_amount: Self) -> Self::Output {
        let other_amount_converted = other_amount.convert(self.immutable_amount.currency());
        let new_immutable_amount = ImmutableAmount::new(
            self.immutable_amount.currency(),
            self.immutable_amount.figure() - other_amount_converted.immutable_amount.figure(),
        );
        Amount {
            immutable_amount: new_immutable_amount,
        }
    }
}
