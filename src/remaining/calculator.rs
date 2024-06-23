use crate::amounts::{Amount, CurrencyIdent, Figure, ExchangeRates};
use crate::period::{AnyPeriodsConfiguration, Period, PeriodsConfiguration};
use chrono::NaiveDate;

pub struct Calculator {
    pub groups: Vec<CalculatorGroup>,
    pub remaining_days: i32
}

impl Calculator {
    fn from_vault_values<P: PeriodsConfiguration>(period_config: &P, date: &NaiveDate) -> Calculator {
        todo!()
    }
    
    fn new<P: PeriodsConfiguration>(groups: Vec<CalculatorGroup>, period_config: &P, date: &NaiveDate) {
        
    }
    
    fn remaining(&self, exchange_rates: ExchangeRates) -> Result<Amount, String> {
        todo!()
    }

    fn remaining_per_day(&self, exchange_rates: ExchangeRates) -> Result<Amount, String>{

    }
}

#[derive(Clone)]
pub enum GroupCombinations {
    ADD,
    SUBTRACT,
}

pub struct CalculatorGroup {
    pub name: String,
    pub combination: GroupCombinations,
    pub entries: Vec<CalculatorEntry>,
}

impl CalculatorGroup {
    fn total(exchange_rates: &ExchangeRates) -> Result<Amount, String>{
        todo!()
    }
}

pub struct CalculatorEntry {
    pub name: String,
    pub amount_timeline: AmountTimeline
}


pub struct AmountTimeline {
    currency_ident: CurrencyIdent,
    period_start: Option<Figure>,
    current: Option<Figure>,
    predicted_end: Option<Figure>,
}

impl AmountTimeline {
    fn make_amount(&self, figure_maybe: &Option<Figure>, exchange_rates: &ExchangeRates) -> Result<Option<Amount>, String>{
        if let Some(figure) = figure_maybe{
            let amount = exchange_rates.new_amount(&self.currency_ident, figure)?;
            Ok(Some(amount))
        } else {
            Ok(None)
        }
    }

    pub fn period_start(&self, exchange_rates: &ExchangeRates) -> Result<Option<Amount>, String>{
        self.make_amount(&self.period_start, &exchange_rates)
    }

    pub fn current(&self, exchange_rates: &ExchangeRates) -> Result<Option<Amount>, String>{
        self.make_amount(&self.current, &exchange_rates)
    }

    pub fn predicted_end(&self, exchange_rates: &ExchangeRates) -> Result<Option<Amount>, String>{
        self.make_amount(&self.predicted_end, &exchange_rates)
    }
}

pub trait CalculatorGroupCollector<E: Into<CalculatorEntry>> {
    const GROUP_COMBINATION: &'static GroupCombinations;

    fn collect_raw<P: PeriodsConfiguration>(
        &self,
        period_config: &P,
        date: &NaiveDate,
    ) -> Result<impl IntoIterator<Item = E>, String>;

    fn collect_converted<P: PeriodsConfiguration>(
        &self,
        period_config: &P,
        date: &NaiveDate,
    ) -> Result<CalculatorGroup, String> {
        Ok(CalculatorGroup {
            combination: Self::GROUP_COMBINATION.clone(),
            entries: self
                .collect_raw(period_config, date)?
                .into_iter()
                .map(E::into)
                .collect(),
        })
    }
}
