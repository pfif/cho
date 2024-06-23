use crate::amounts::{Amount};
use std::fmt::Display;

pub struct DisplayAmount<'a> {
    /*exchange_rates: &'a ExchangeRates,
    currency_ident: &'a CurrencyIdent,
    figure: &'a Figure,*/
    raw: Amount<'a>
}

impl<'a> From<Amount<'a>> for DisplayAmount<'a>{
    fn from(value: Amount) -> Self {
        DisplayAmount{
            raw: value
        }
    }
}

impl<'a> Display for DisplayAmount<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sign = &self.raw.currency.sign;

        write!(f, "{}{}", sign, self.raw.figure)
    }
}