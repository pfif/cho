use std::fs::{read_dir, File};
use chrono::NaiveDate;
#[cfg(test)]
use mockall::automock;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::from_reader;
use crate::period::Period;
use crate::remaining_operation::amounts::Amount;
use crate::remaining_operation::amounts::exchange_rates::ExchangeRates;
use crate::remaining_operation::core_types::{GroupBuilder, Operand, OperandBuilder};
use crate::remaining_operation::operand_builders::timeline::{TimelineOperandBuilder, TimelineOperandEnd};
use crate::vault::Vault;

// Public traits
pub type Figure = u32;
const ACCOUNT_DIR: &str = "accounts";

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct FoundAmount {
    pub figure: Figure,
    pub estimated: bool,
}

impl FoundAmount {
    // TODO This method is terribly named, but its need will be gone once we align the codebase to use Amount everywhere.
    fn into_remaining_module_amount(self, currency: &String, exchange_rates: &ExchangeRates) -> Result<Amount, String> {
        exchange_rates.new_amount(currency, Decimal::from(self.figure))
    }
}

#[cfg_attr(test, automock)]
pub trait QueriableAccount {
    fn amount_at(&self, date: &NaiveDate) -> Result<FoundAmount, String>;
    fn name(&self) -> &String;
    fn currency(&self) -> &String;
}


// TODO - This should read the account from the Vault, otherwise this is breaking the abstraction of
//        however we choose to store "state"
pub fn get_accounts<V: Vault>(vault: &V) -> Result<Vec<AccountJson>, String> {
    let directory = vault.path();
    let dir_reader = match read_dir(directory.join(ACCOUNT_DIR)) {
        Err(why) => {
            return Err("Could not read the Accounts directory: ".to_string() + &why.to_string())
        }
        Ok(reader) => reader,
    };

    let mut accounts: Vec<AccountJson> = Vec::new();

    for maybe_dir_entry in dir_reader {
        let dir_entry =
            maybe_dir_entry.map_err(|why| return format!("Could not read file: {}", why))?;

        let path = dir_entry.path();
        let path_str = if let Some(s) = &(path).to_str() {
            s
        } else {
            "(unable to get filename)"
        };

        let file_type = dir_entry.file_type().map_err(|why| {
            return format!("Could not read the file type of {}: {}", path_str, why);
        })?;

        if file_type.is_file() {
            let file = match File::open(&path) {
                Err(why) => return Err(format!("Could not read file {}: {}", path_str, why)),
                Ok(file) => file,
            };

            let account: AccountJson = match from_reader(file) {
                Err(why) => {
                    return Err(format!(
                        "Could not parse account for file {}: {}",
                        path_str, why
                    ))
                }
                Ok(file) => file,
            };

            accounts.push(account);
        }
    }

    return Ok(accounts);
}

pub struct AccountGetter{
    accounts: Vec<AccountJson>
}

impl AccountGetter {
    pub fn from_files<V: Vault>(vault: &V) -> Result<AccountGetter, String>{
        Ok(AccountGetter{
            accounts: get_accounts(vault)?
        })
    }
}

impl Into<GroupBuilder> for AccountGetter {
    fn into(self) -> GroupBuilder {
        GroupBuilder{
            name: "Accounts".into(),
            operand_factories: self.accounts.into_iter().map(|account| Box::new(account) as Box<dyn OperandBuilder>).collect()
        }
    }
}

#[allow(non_snake_case)]
#[cfg(test)]
mod tests_get_accounts {
    use chrono::NaiveDate;
    use std::collections::HashSet;
    use std::fs::{create_dir, File};
    use std::io::prelude::*;
    use std::path::{Path, PathBuf};
    use tempfile::{tempdir, TempDir};

    use crate::accounts::{get_accounts, AccountJson, AmountListItem, ACCOUNT_DIR};
    use crate::vault::Vault;

    struct MockVault {
        path: PathBuf,
    }

    impl Vault for MockVault {
        fn path(&self) -> &PathBuf {
            return &self.path;
        }

        fn read_vault_values<T: serde::de::DeserializeOwned>(
            &self,
            _name: String,
        ) -> Result<T, String> {
            todo!()
        }
    }

    fn create_account_file(directory: &TempDir, name: &str, content: &str) {
        let path = Path::join(&Path::join(directory.path(), ACCOUNT_DIR), name);
        let mut file = File::create(path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn parse_accounts__nominal() {
        let directory = tempdir().unwrap();

        create_dir(Path::join(directory.path(), ACCOUNT_DIR)).unwrap();

        let raw_account_left = r#"{
"name": "account_left",
"currency": "JPY",
"amounts": [
    {"date": "2023-04-08", "amount": 55000},
    {"date": "2023-04-10", "amount": 53000},
    {"date": "2023-04-12", "amount": 60000}
]
}"#;
        create_account_file(&directory, "account_left.json", raw_account_left);

        let raw_account_right = r#"{
"name": "account_right",
"currency": "EUR",
"amounts": [
    {"date": "2023-02-03", "amount": 5000},
    {"date": "2023-03-13", "amount": 5200},
    {"date": "2023-05-16", "amount": 6000}
]
}"#;
        create_account_file(&directory, "account_right.json", raw_account_right);

        let expected_accounts = HashSet::from([
            AccountJson {
                name: "account_left".to_string(),
                currency: String::from("JPY"),
                amounts: vec![
                    AmountListItem {
                        date: NaiveDate::from_ymd_opt(2023, 04, 08).unwrap(),
                        amount: 55000,
                    },
                    AmountListItem {
                        date: NaiveDate::from_ymd_opt(2023, 04, 10).unwrap(),
                        amount: 53000,
                    },
                    AmountListItem {
                        date: NaiveDate::from_ymd_opt(2023, 04, 12).unwrap(),
                        amount: 60000,
                    },
                ],
            },
            AccountJson {
                name: "account_right".to_string(),
                currency: String::from("EUR"),
                amounts: vec![
                    AmountListItem {
                        date: NaiveDate::from_ymd_opt(2023, 02, 03).unwrap(),
                        amount: 5000,
                    },
                    AmountListItem {
                        date: NaiveDate::from_ymd_opt(2023, 03, 13).unwrap(),
                        amount: 5200,
                    },
                    AmountListItem {
                        date: NaiveDate::from_ymd_opt(2023, 05, 16).unwrap(),
                        amount: 6000,
                    },
                ],
            },
        ]);

        let vault = MockVault {
            path: directory.path().to_path_buf(),
        };

        assert_eq!(
            HashSet::from_iter(get_accounts(&vault).unwrap()),
            expected_accounts
        )
    }
}

// JSON implementation
#[derive(Deserialize, Hash, Eq, PartialEq, Debug)]
pub struct AccountJson {
    name: String,
    currency: String,
    amounts: Vec<AmountListItem>,
}

// TODO - Unit tests for this
impl OperandBuilder for AccountJson {
    fn build(&self, period: &Period, today: &NaiveDate, exchange_rates: &ExchangeRates) -> Result<Operand, String> {
        let start_amount = self.amount_at(&period.start_date)?.into_remaining_module_amount(self.currency(), exchange_rates)?;
        let end_amount = self.amount_at(&period.end_date)?.into_remaining_module_amount(self.currency(), exchange_rates)?;

        let builder = TimelineOperandBuilder{
            name: self.name.clone(),
            start_amount,
            wrapper_end_amount: TimelineOperandEnd::Current(end_amount)
        };
        builder.build(period, today, exchange_rates)
    }
}

#[derive(Deserialize, Hash, Eq, PartialEq, Debug)]
pub struct AmountListItem {
    date: NaiveDate,
    amount: Figure,
}

impl QueriableAccount for AccountJson {
    /// This function takes a date and returns the amount that was
    /// available on the account on that date.
    ///
    /// The function searches through the account's amount history. If
    /// an amount was recorded for the date, it returns that amount.
    /// Otherwise, it returns the last recorded amount before that
    /// date.
    ///
    /// If no amount was recorded for the passed date, the
    /// FoundAmount's `estimated` field is set to true.
    fn amount_at(&self, date: &NaiveDate) -> Result<FoundAmount, String> {
        let mut iter = self.amounts.iter().peekable();

        let Some(mut item_left) = &iter.next() else {
            return Err("The account has no amount history".to_string());
        };

        if *date < item_left.date {
            return Err("The requested date is before the start of the amount history".to_string());
        }

        loop {
            let date_between_left_and_right = match &iter.peek() {
                Some(item_right) => {
                    if item_left.date > item_right.date {
                        return Err("Amount history out of order".to_string());
                    }

                    *date > item_left.date && *date < item_right.date
                }
                None => true,
            };

            if *date == item_left.date {
                return Ok(FoundAmount {
                    figure: item_left.amount,
                    estimated: false,
                });
            }

            if date_between_left_and_right {
                return Ok(FoundAmount {
                    figure: item_left.amount,
                    estimated: true,
                });
            }

            item_left = match &iter.next() {
                Some(v) => v,
                None => return Err("Reached end of the list but did not return amount".to_string()),
            }
        }
    }

    fn name(&self) -> &String {
        return &self.name;
    }

    fn currency(&self) -> &String {
        return &self.currency;
    }
}

#[cfg(test)]
mod tests_accountjson_amount_at {
    use chrono::NaiveDate;

    use super::{AccountJson, AmountListItem, Figure, FoundAmount, QueriableAccount};

    fn date(day: u32) -> NaiveDate {
        return NaiveDate::from_ymd_opt(1995, 5, day).unwrap();
    }

    fn list_in_order() -> Vec<AmountListItem> {
        return Vec::from([
            AmountListItem {
                date: date(15),
                amount: 1500,
            },
            AmountListItem {
                date: date(19),
                amount: 1800,
            },
        ]);
    }

    fn list_out_of_order() -> Vec<AmountListItem> {
        return Vec::from([
            AmountListItem {
                date: date(19),
                amount: 1800,
            },
            AmountListItem {
                date: date(15),
                amount: 1500,
            },
        ]);
    }

    fn sample_account(list: Vec<AmountListItem>) -> AccountJson {
        return AccountJson {
            name: "Test account".to_string(),
            currency: String::from("EN"),
            amounts: list,
        };
    }

    fn assert_correct(day: u32, amount: Figure, estimated: bool) {
        assert_eq!(
            sample_account(list_in_order()).amount_at(&date(day)),
            Result::Ok(FoundAmount {
                figure: amount,
                estimated: estimated
            })
        )
    }

    #[test]
    fn before_start() {
        assert_eq!(
            sample_account(list_in_order()).amount_at(&date(13)),
            Result::Err("The requested date is before the start of the amount history".to_string())
        )
    }

    #[test]
    fn first_date() {
        assert_correct(15, 1500, false)
    }

    #[test]
    fn between_date() {
        assert_correct(17, 1500, true)
    }

    #[test]
    fn second_date() {
        assert_correct(19, 1800, false)
    }

    #[test]
    fn after_last_date() {
        assert_correct(22, 1800, true)
    }

    fn assert_out_of_order(day: u32) {
        assert_eq!(
            sample_account(list_out_of_order()).amount_at(&date(day)),
            Result::Err("Amount history out of order".to_string())
        )
    }

    #[test]
    fn error_list_out_of_order_first_date() {
        assert_out_of_order(19);
    }

    #[test]
    fn error_list_out_of_order_later_date() {
        assert_out_of_order(22);
    }
}
