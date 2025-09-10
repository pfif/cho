use std::fmt::{Debug, Display, Formatter};
use crate::remaining_operation::amounts::amount::ImmutableAmount;
use rust_decimal::Decimal;
use std::ops;

pub type Figure = Decimal;
pub type CurrencyIdent = String;
pub type Sign = String;

// TODO To make this code more efficient, to make sure we keep only one version of the currency in memory and as an exercice for me to understand lifetimes( ??),
//      remove the clone from here and use lifetimes.
//      We shouldn't need more than one instance per currency
//      Ian told me that Rc could be used for this use-case
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Currency {
    pub rate: Figure,
    pub sign: String,
}

pub mod exchange_rates {
    use super::{Amount, Currency, CurrencyIdent, Figure};
    use crate::remaining_operation::amounts::amount::ImmutableAmount;
    use std::collections::HashMap;

    #[derive(Clone)]
    pub struct ExchangeRates {
        currencies: HashMap<CurrencyIdent, Currency>,
    }

    impl ExchangeRates {
        pub fn from_indent_and_rates(rates: Vec<(CurrencyIdent, Figure)>) -> Result<ExchangeRates, String> {
            let currencies: Result<HashMap<CurrencyIdent, Currency>, String> = rates
                .into_iter()
                .map(|(ident, rate)| {
                    let sign = match ident.as_str() {
                        "EUR" => "€".to_string(),
                        "JPY" => "¥".to_string(),
                        _ => return Err(format!(
                            "Unsupported currency: {}. We support only EUR and JPY for now.", ident)),
                    };

                    Ok((ident, Currency { rate, sign }))
                })
                .collect();

            currencies.map(|rate| ExchangeRates { currencies: rate })
        }
        pub fn get_currency(&self, ident: &CurrencyIdent) -> Result<&Currency, String> {
            self.currencies
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

    #[cfg(test)]
    impl ExchangeRates {
        pub fn from_indent_rates_and_sign(rates: Vec<(CurrencyIdent, Figure, String)>) -> ExchangeRates {
            let currencies: HashMap<CurrencyIdent, Currency> = rates
                .into_iter()
                .map(|(ident, rate, sign)| {
                    (ident, Currency { rate, sign })
                })
                .collect();

            ExchangeRates { currencies }
        }
    }
}

mod amount {
    use crate::remaining_operation::amounts::{Currency, Figure};

    #[derive(Clone, Debug, Eq, PartialEq)]
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
#[derive(Clone, PartialEq, Eq)]
pub struct Amount {
    immutable_amount: ImmutableAmount,
}

#[cfg(test)]
impl Amount {
    pub(crate) fn new_mock(currency: &Currency, figure: Figure) -> Amount {
        Amount {
            immutable_amount: ImmutableAmount::new(currency, figure),
        }
    } 
}

impl Display for Amount {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.immutable_amount.currency().sign, self.immutable_amount.figure())
    }
}

impl Debug for Amount {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
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