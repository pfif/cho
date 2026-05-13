use crate::remaining_operation::core_types::{
    IllustrationValue, RemainingOperationScreen, RemainingOperationScreenGroup,
};
use comfy_table::Table;

const NO_OPERAND_MESSAGE: &str = "No operands for this period";

pub fn format_remaining_operation_screen(screen: &RemainingOperationScreen) -> String {
    let mut components = vec![title(&format!(
        "Current period : {} to {}",
        screen.period.start_date, screen.period.end_date,
    ))];

    for group in screen.groups.iter() {
        components.push(render_group(&group));
    }

    components.push(title(&format!(
        "Remaining this period: {}",
        screen.remaining
    )));

    components.push(format!("Release: {}", env!("RELEASE")));

    format!("{}\n", components.join("\n\n"))
}

fn render_group(group: &RemainingOperationScreenGroup) -> String {
    let group_title = title(&group.name);

    let table = if !group.operands.is_empty() {
        let rendered = render_group_table(group);
        Some(rendered)
    } else {
        None
    };

    let sentence = if !group.archived_operand_with_non_zero_amounts.is_empty() {
        Some(render_archive_operand_with_non_zero_amounts_warning(&group.archived_operand_with_non_zero_amounts))
    } else if group.operands.is_empty() {
        Some(NO_OPERAND_MESSAGE.to_string())
    } else {
        None
    };

    let content = [table, sentence]
        .into_iter()
        .filter_map(|x| x)
        .collect::<Vec<String>>()
        .join("\n");
    format!("{group_title}\n{content}")
}

fn render_group_table(group: &RemainingOperationScreenGroup) -> String {
    let mut table = Table::new();

    let mut illustration_fields = vec![String::from("Name"), String::from("Amount")];
    illustration_fields.extend(group.illustration_fields.clone());
    table.set_header(illustration_fields);

    for operand in group.operands.iter() {
        let mut illustration_values = vec![operand.name.clone(), operand.amount.to_string()];

        let raw_illustration_value = operand
            .illustration
            .clone()
            .into_iter()
            .map(|(_, value)| value)
            .map(|illustration_value| match illustration_value {
                IllustrationValue::Amount(amount) => amount.to_string(),
                IllustrationValue::Null => "-".to_string(),
                IllustrationValue::Bool(bool) => (if bool { "✅" } else { "" }).into(),
                IllustrationValue::Date(date) => date.to_string(),
            });

        illustration_values.extend(raw_illustration_value);
        table.add_row(illustration_values);
    }

    let mut total_row = vec!["Total".to_string(), group.total.to_string()];
    total_row.extend(group.illustration_fields.iter().map(|_| "".to_string()));
    table.add_row(total_row);

    let rendered = table.to_string();
    rendered
}

fn title(string: &str) -> String {
    let string_length = string.len();
    string.to_string() + "\n" + &"=".repeat(string_length)
}

fn render_archive_operand_with_non_zero_amounts_warning(
    operand_names: &Vec<String>
) -> String {
    let sentence = if let Some((last, other)) = operand_names.split_last() {
        let all_but_one_joined_by_string = other.join(", ");
        if all_but_one_joined_by_string != "" {
            format!("{all_but_one_joined_by_string} and {last} have been archived, but their current amount is not zero and still impact the group's total.")
        } else {
            format!("{last} has been archived, but its current amount is not zero and still impact the group's total.")
        }
    } else {
        "".to_string()
    };

    format!("⚠ {sentence}")
}

#[cfg(test)]
mod tests {
    mod render_group {
        use crate::amounts::exchange_rates::ExchangeRates;
        use crate::cli::formatting::{
            render_archive_operand_with_non_zero_amounts_warning, render_group, render_group_table,
            title, NO_OPERAND_MESSAGE,
        };
        use crate::remaining_operation::core_types::{
            IllustrationValue, RemainingOperationScreenGroup, RemainingOperationScreenOperand,
        };
        use pretty_assertions::assert_eq;

        #[test]
        fn render_group_table_only() {
            let exchange_rates = ExchangeRates::for_tests();
            let (operand, illustration_fields) = create_operand(&exchange_rates);
            let (group, rendered_title) =
                create_group(exchange_rates, vec![operand], illustration_fields, vec![]);

            let result = render_group(&group);
            let rendered_table = render_group_table(&group);
            let expected = format!(
                "\
{rendered_title}
{rendered_table}"
            );
            assert_eq!(result, expected);
        }

        #[test]
        fn render_group_table_and_archive_warning() {
            let exchange_rates = ExchangeRates::for_tests();
            let (operand, illustration_fields) = create_operand(&exchange_rates);
            let (group, rendered_title) = create_group(
                exchange_rates,
                vec![operand],
                illustration_fields,
                vec!["Banque Populaire".to_string()],
            );

            let result = render_group(&group);
            let rendered_table = render_group_table(&group);
            let rendered_warning = render_archive_operand_with_non_zero_amounts_warning(&group.archived_operand_with_non_zero_amounts);
            let expected = format!(
                "\
{rendered_title}
{rendered_table}
{rendered_warning}"
            );
            assert_eq!(result, expected);
        }

        #[test]
        fn render_archive_warning_only() {
            let exchange_rates = ExchangeRates::for_tests();
            let (group, rendered_title) = create_group(
                exchange_rates,
                vec![],
                vec![],
                vec!["Banque Populaire".to_string()],
            );

            let result = render_group(&group);
            let rendered_warning = render_archive_operand_with_non_zero_amounts_warning(&group.archived_operand_with_non_zero_amounts);
            let expected = format!(
                "\
{rendered_title}
{rendered_warning}"
            );
            assert_eq!(result, expected);
        }

        #[test]
        fn render_archive_empty() {
            let exchange_rates = ExchangeRates::for_tests();
            let (group, rendered_title) = create_group(exchange_rates, vec![], vec![], vec![]);

            let result = render_group(&group);
            let expected = format!(
                "\
{rendered_title}
{NO_OPERAND_MESSAGE}"
            );
            assert_eq!(result, expected);
        }

        fn create_operand(
            exchange_rates: &ExchangeRates,
        ) -> (RemainingOperationScreenOperand, Vec<String>) {
            let (operand, illustration_fields) = (
                RemainingOperationScreenOperand {
                    name: "Credit Agricole".to_string(),
                    amount: exchange_rates.euro("5"),
                    illustration: vec![("Legal".to_string(), IllustrationValue::Bool(true))],
                },
                vec!["Legal".to_string()],
            );
            (operand, illustration_fields)
        }

        fn create_group(
            exchange_rates: ExchangeRates,
            operands: Vec<RemainingOperationScreenOperand>,
            illustration_fields: Vec<String>,
            archived_operand_with_non_zero_amounts: Vec<String>,
        ) -> (RemainingOperationScreenGroup, String) {
            let name = "Accounts".to_string();
            let (group, rendered_title) = (
                RemainingOperationScreenGroup {
                    name: name.clone(),
                    operands,
                    illustration_fields,
                    total: exchange_rates.euro("5"),
                    archived_operand_with_non_zero_amounts,
                },
                title(&name),
            );
            (group, rendered_title)
        }
    }

    mod render_archive_operand_with_non_zero_amounts_warning {
        use crate::cli::formatting::render_archive_operand_with_non_zero_amounts_warning;

        #[test]
        fn one() {
            let result = render_archive_operand_with_non_zero_amounts_warning(
                &vec!["Banque Populaire".to_string()]);
            let expected = "⚠ Banque Populaire has been archived, but its current amount is not zero and still impact the group's total.";
            assert_eq!(result, expected);
        }

        #[test]
        fn two() {
            let result = render_archive_operand_with_non_zero_amounts_warning(
                &vec![
                    "Banque Populaire".to_string(),
                    "LINE".to_string()
                ]);
            let expected = "⚠ Banque Populaire and LINE have been archived, but their current amount is not zero and still impact the group's total.";
            assert_eq!(result, expected);
        }
        #[test]
        fn three() {
            let result = render_archive_operand_with_non_zero_amounts_warning(
                &vec![
                    "Paypay".to_string(),
                    "Banque Populaire".to_string(),
                    "LINE".to_string()
                ]);
            let expected = "⚠ Paypay, Banque Populaire and LINE have been archived, but their current amount is not zero and still impact the group's total.";
            assert_eq!(result, expected);
        }
    }
}
