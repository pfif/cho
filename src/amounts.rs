use std::fmt::{Debug, Display, Formatter};
use crate::amounts::amount::ImmutableAmount;
use rust_decimal::Decimal;
use std::ops;
use serde::Deserialize;

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
    use super::{Amount, Currency, CurrencyIdent, Figure, RawAmount};
    use crate::amounts::amount::ImmutableAmount;
    use std::collections::HashMap;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

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
        
        pub fn new_amount_from_raw_amount(
            &self,
            raw_amount: &RawAmount,
        ) -> Result<Amount, String>{
           self.new_amount(&raw_amount.currency, raw_amount.figure)
        }
    }

    #[cfg(test)]
    impl ExchangeRates {
        pub fn for_tests() -> ExchangeRates {
            ExchangeRates::from_indent_and_rates(vec![
                ("EUR".to_string(), dec!(1)),
                ("JPY".to_string(), dec!(2))
            ]).expect("Can create exchange rates")
        }
        
        pub fn yen(&self, figure: &str) -> Amount {
            self.new_amount(
                &"JPY".to_string(),
                Decimal::from_str_exact(figure).expect("can build a decimal from passed string")
            ).expect("Can create an amount")
        }
        
        pub fn euro(&self, figure: &str) -> Amount {
            self.new_amount(
                &"EUR".to_string(),
                Decimal::from_str_exact(figure).expect("can build a decimal from passed string")
            ).expect("Can create an amount")
        }
    }
}

mod amount {
    use crate::amounts::{Currency, Figure};

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
            self.immutable_amount.figure() * exchange_rate
        );

        Amount {
            immutable_amount: new_immutable_amount,
        }
    }
}

impl Amount {
    pub fn add(&self, other_amount: &Amount) -> Amount {
        let other_amount_converted = other_amount.convert(self.immutable_amount.currency());
        let new_immutable_amount = ImmutableAmount::new(
            self.immutable_amount.currency(),
            self.immutable_amount.figure() + other_amount_converted.immutable_amount.figure(),
        );
        Amount {
            immutable_amount: new_immutable_amount,
        }
    }

    pub fn sub(&self, other_amount: &Amount) -> Amount {
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

#[derive(Deserialize)]
pub struct RawAmount {
    pub currency: CurrencyIdent,
    pub figure: Figure,
}

impl RawAmount {
    pub fn to_amount(&self, exchange_rates: &exchange_rates::ExchangeRates) -> Result<Amount, String> {
        exchange_rates.new_amount(&self.currency, self.figure)
    }
}


#[cfg(test)]
impl RawAmount {
    pub fn yen(figure: &str) -> RawAmount {
        RawAmount {
            currency: "JPY".to_string(),
            figure: Decimal::from_str_exact(figure).expect("can build a decimal from passed string"),
        }
    }
}