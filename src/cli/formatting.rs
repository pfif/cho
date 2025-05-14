use comfy_table::Table;
use crate::remaining_operation::core_types::{IllustrationValue, RemainingOperationScreen};

pub fn format_remaining_operation_screen(screen: &RemainingOperationScreen) -> String {
    let mut components = vec![title(&format!(
        "Current period : {} to {}", screen.period.start_date, screen.period.end_date,
    ))];

    for group in screen.groups.iter() {
        let mut table = Table::new();
        let group_title = title(group.name());

        // TODO - do we need a column that shows the number used for the math?
        let mut illustration_fields = vec![String::from("Name")];
        illustration_fields.extend(group.illustration_fields());
        table.set_header(illustration_fields);

        for operand in group.operands() {
            let mut illustration_values = vec![operand.name.clone()];

            let raw_illustration_value = operand.illustration.clone()
                .into_iter().map(|(_, value)| value)
                .map(|illustration_value| {
                    match illustration_value {
                        IllustrationValue::Amount(amount) => format!("{}", amount),
                        IllustrationValue::Bool(bool) => (if bool { "✅" } else { "" }).into()
                    }
                });

            illustration_values.extend(raw_illustration_value);
            table.add_row(illustration_values);
        }

        components.push(format!("{}\n{}", group_title, table.to_string()));
    }

    components.push(title(&format!("Remaining this period: {}", screen.remaining)));
    
    components.push(format!("Release: {}", env!("RELEASE")));

    components.join("\n\n")
}

fn title(string: &str) -> String {
    let string_length = string.len();
    string.to_string() + "\n" + &"=".repeat(string_length)
}