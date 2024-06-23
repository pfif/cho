/*use crate::amounts::{CurrencyIdent, ExchangeRates, Figure};
use crate::remaining::{Calculator, CalculatorEntry, CalculatorGroup};
use comfy_table::Table;
use std::fmt::{Display, Formatter};

struct DisplayCalculator {}

impl Display for DisplayCalculator {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

struct DisplayCalculatorGroup<'a> {
    raw: CalculatorGroup,
    exchange_rates: CliExchangeRates<'a>
}

impl<'a> Display for DisplayCalculatorGroup<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // TODO Print combination and name
        let mut table = Table::new();
        table.set_header(vec!["name", "period_start", "current", "predicted_end"]);

        for entry in self.raw.entries {
            table.add_row(vec![
                entry.name,
                entry.period_start.
            ]);
        };
    }
}*/