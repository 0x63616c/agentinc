//! Pieces the board and the list share, so a Ticket reads the same in both.
use super::*;

/// The "Working" mark on a Ticket with an active run.
pub(super) fn working_indicator() -> Div {
    row()
        .gap(px(SPACE_1))
        .child(status_dot(Tone::Info))
        .child(hint("Working"))
}

/// A status group's icon, name and count, for a board lane and a list group.
/// The caller supplies the container and any trailing action.
pub(super) fn status_group_header(status: TicketStatus, count: usize) -> Div {
    row()
        .gap(px(SPACE_2))
        .child(icon(status_icon(status), ICON_SIZE_SM).text_color(rgb(status_color(status))))
        .child(
            div()
                .text_size(type_size(LABEL_SIZE))
                .font_weight(FontWeight::MEDIUM)
                .child(status_name(status)),
        )
        .child(hint(count.to_string()))
}

/// A card's head and title: its key, a "Working" mark, the assignee's avatar
/// and the clamped title. The board card and the card that follows the pointer
/// while dragging both start with it.
pub(super) fn ticket_card_body(
    key: SharedString,
    title: SharedString,
    running: bool,
    assignee: &AssigneeFace,
) -> Div {
    column()
        .w_full()
        .gap(px(SPACE_2))
        .child(
            row()
                .w_full()
                .gap(px(SPACE_2))
                .child(hint(key))
                .when(running, |s| s.child(working_indicator()))
                .child(div().flex_1())
                .child(assignee_avatar(assignee, AVATAR_SIZE_SM)),
        )
        .child(
            div()
                .w_full()
                .text_size(type_size(BODY_SIZE))
                .line_height(relative(TITLE_LINE_HEIGHT))
                .text_color(rgb(TEXT))
                .line_clamp(3)
                .text_ellipsis()
                .child(title),
        )
}
