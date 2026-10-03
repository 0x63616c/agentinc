//! Sentence builders for the copy rules in docs/design-system.md: errors,
//! counts and destructive confirmations read the same on every page.

/// A load error: `{Thing} is unavailable. {Recovery}.`
pub fn unavailable(thing: &str, recovery: &str) -> String {
    format!(
        "{thing} is unavailable. {}.",
        recovery.trim_end_matches('.')
    )
}

/// `1 Ticket`, `3 Tickets`, `No Tickets`.
pub fn pluralize(n: usize, one: &str, many: &str) -> String {
    match n {
        0 => format!("No {many}"),
        1 => format!("1 {one}"),
        n => format!("{n} {many}"),
    }
}

/// The destructive confirmation dialog: `(title, body, button)`.
/// `what` names what goes, capitalized as the start of a sentence.
pub fn confirm_delete(name: &str, what: &str) -> (String, String, &'static str) {
    (
        format!("Delete “{name}”?"),
        format!("{what} will be permanently deleted."),
        "Delete",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_name_the_thing_and_the_way_out() {
        assert_eq!(
            unavailable("Tickets", "Refresh to try again"),
            "Tickets is unavailable. Refresh to try again."
        );
        assert_eq!(
            unavailable("Temporal", "Try again."),
            "Temporal is unavailable. Try again."
        );
    }

    #[test]
    fn counts_agree_with_their_noun() {
        assert_eq!(pluralize(0, "Ticket", "Tickets"), "No Tickets");
        assert_eq!(pluralize(1, "Ticket", "Tickets"), "1 Ticket");
        assert_eq!(pluralize(2, "Ticket", "Tickets"), "2 Tickets");
    }

    #[test]
    fn delete_dialogs_follow_the_style_sheet() {
        assert_eq!(
            confirm_delete(
                "Plan the week",
                "This Ticket, its Comments and its relationships"
            ),
            (
                "Delete “Plan the week”?".to_owned(),
                "This Ticket, its Comments and its relationships will be permanently deleted."
                    .to_owned(),
                "Delete"
            )
        );
    }
}
