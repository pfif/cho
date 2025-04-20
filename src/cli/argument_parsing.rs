use rust_decimal::Decimal;
use clap::Parser;
use std::path::PathBuf;

fn parse_exchange_rate(s: &str) -> Result<(String, Decimal), String> {
    let rate: Option<(String, Decimal)> = (|| {
        let splitted_string = s.split(":").collect::<Vec<&str>>();
        if splitted_string.len() != 2 {
            return None;
        }

        let currency = splitted_string[0];
        let raw_rate = splitted_string[1];

        let rate = Decimal::from_str_exact(raw_rate).ok()?;

        Some((currency.into(), rate))
    })();

    rate.ok_or(format!(
        "Could not decode exchange rate {}: Format is {{CURRENCY_NAME}}:{{RATE}}, eg. EUR:0.24561",
        &s
    ))
}

#[derive(Parser)]
#[command()]
pub struct RemainingOptions {
    #[arg(short = 'r', long = "exchange-rate", value_parser = parse_exchange_rate)]
    pub exchange_rates: Vec<(String, Decimal)>,

    #[arg(short = 't', long = "target-currency")]
    pub target_currency: String,

    #[arg(short = 'p', long = "include-predicted")]
    pub include_predicted_income: bool,

    #[arg(short = 'V', long)]
    pub vault: Option<PathBuf>,
}