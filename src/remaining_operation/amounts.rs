use rust_decimal::Decimal;
use std::ops;

pub type Figure = Decimal;
pub type CurrencyIdent = String;
pub type Sign = String;

// TODO To make this code more efficient, and as an exercice for me to understand lifetimes,
//      remove the clone from here and use lifetimes.
//      We shouldn't need more than one instance per currency
#[derive(Clone)]
pub struct Currency {
    pub rate: Figure,
    pub sign: String,
}

pub mod exchange_rates {
    use super::{Amount, Currency, CurrencyIdent, Figure};
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
            let currency = self.get_currency(currency_ident)?.clone();
            Ok(Amount { currency, figure })
        }
    }
}

pub struct Amount {
    pub currency: Currency,
    pub figure: Figure,
}

impl Amount {
    fn change_figure<F>(&self, mathematicer: F) -> Amount
    where
        F: Fn(&Figure) -> Figure,
    {
        let new_figure = mathematicer(&self.figure).round_dp_with_strategy(2, rust_decimal::RoundingStrategy::MidpointNearestEven);
        Amount {
            currency: self.currency.clone(),
            figure: new_figure,
        }
    }
}

impl ops::Add for &Amount {
    type Output = Amount;

    fn add(self, other_amount: Self) -> Self::Output {
        let exchange_rate = {
            let target_currency_value = self.currency.rate;
            let from_currency_value = other_amount.currency.rate;
            target_currency_value / from_currency_value
        };

        self.change_figure(|our_figure| {
            our_figure + (other_amount.figure * exchange_rate)
        })
    }
}