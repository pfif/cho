use rust_decimal::Decimal;
use std::collections::HashMap;

pub type Figure = Decimal;
pub type CurrencyIdent = String;
pub type Sign = String;

pub struct Currency {
    pub rate: Figure,
    pub sign: String,
}

pub struct ExchangeRates{
    rate: HashMap<CurrencyIdent, Currency>
}

impl ExchangeRates {
    pub fn new_amount(&self, currency_ident: &CurrencyIdent, figure: &Figure) -> Result<Amount, String>{
        let currency = self.rate.get(currency_ident).ok_or(format!("Could not find currency ident: {}", currency_ident))?;
        Ok(Amount{
            currency,
            figure
        })
    }
}

pub struct Amount<'a>{
    pub currency: &'a Currency,
    pub figure: &'a Figure,
}