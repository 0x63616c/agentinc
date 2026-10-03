//! The icon catalogue: one variant per embedded glyph, so a missing icon is a
//! compile error rather than an empty asset at run time.
use gpui::SharedString;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    Refresh,
    Temporal,
    Check,
    ChevronDown,
    ChevronUpDown,
    More,
    User,
    Users,
    Help,
    Feedback,
    Download,
    Inbox,
    Warning,
    Info,
    Copy,
    Trash,
    Edit,
    Play,
    Pause,
    Stop,
    History,
    Repeat,
    Command,
    StatusBacklog,
    StatusTodo,
    StatusProgress,
    StatusBlocked,
    StatusDone,
    StatusCancelled,
    PriorityUrgent,
    PriorityHigh,
    PriorityMedium,
    PriorityLow,
    PriorityNone,
    Dot,
    Link,
    Tag,
    Filter,
    Board,
    List,
    Send,
    OpenAi,
    Agents,
    ArrowRight,
    ArrowUpRight,
    ChevronLeft,
    ChevronRight,
    Close,
    EveeOutline,
    Panel,
    Plus,
    Search,
    Settings,
    Spark,
    Tasks,
    Terminal,
}

impl Icon {
    /// Every icon, for the asset source's lookup by path.
    pub const ALL: [Icon; 56] = [
        Icon::Refresh,
        Icon::Temporal,
        Icon::Check,
        Icon::ChevronDown,
        Icon::ChevronUpDown,
        Icon::More,
        Icon::User,
        Icon::Users,
        Icon::Help,
        Icon::Feedback,
        Icon::Download,
        Icon::Inbox,
        Icon::Warning,
        Icon::Info,
        Icon::Copy,
        Icon::Trash,
        Icon::Edit,
        Icon::Play,
        Icon::Pause,
        Icon::Stop,
        Icon::History,
        Icon::Repeat,
        Icon::Command,
        Icon::StatusBacklog,
        Icon::StatusTodo,
        Icon::StatusProgress,
        Icon::StatusBlocked,
        Icon::StatusDone,
        Icon::StatusCancelled,
        Icon::PriorityUrgent,
        Icon::PriorityHigh,
        Icon::PriorityMedium,
        Icon::PriorityLow,
        Icon::PriorityNone,
        Icon::Dot,
        Icon::Link,
        Icon::Tag,
        Icon::Filter,
        Icon::Board,
        Icon::List,
        Icon::Send,
        Icon::OpenAi,
        Icon::Agents,
        Icon::ArrowRight,
        Icon::ArrowUpRight,
        Icon::ChevronLeft,
        Icon::ChevronRight,
        Icon::Close,
        Icon::EveeOutline,
        Icon::Panel,
        Icon::Plus,
        Icon::Search,
        Icon::Settings,
        Icon::Spark,
        Icon::Tasks,
        Icon::Terminal,
    ];

    /// The asset path GPUI loads this glyph from.
    pub fn path(self) -> &'static str {
        match self {
            Icon::Refresh => "refresh.svg",
            Icon::Temporal => "temporal.svg",
            Icon::Check => "check.svg",
            Icon::ChevronDown => "chevronDown.svg",
            Icon::ChevronUpDown => "chevronUpDown.svg",
            Icon::More => "more.svg",
            Icon::User => "user.svg",
            Icon::Users => "users.svg",
            Icon::Help => "help.svg",
            Icon::Feedback => "feedback.svg",
            Icon::Download => "download.svg",
            Icon::Inbox => "inbox.svg",
            Icon::Warning => "warning.svg",
            Icon::Info => "info.svg",
            Icon::Copy => "copy.svg",
            Icon::Trash => "trash.svg",
            Icon::Edit => "edit.svg",
            Icon::Play => "play.svg",
            Icon::Pause => "pause.svg",
            Icon::Stop => "stop.svg",
            Icon::History => "history.svg",
            Icon::Repeat => "repeat.svg",
            Icon::Command => "command.svg",
            Icon::StatusBacklog => "status-backlog.svg",
            Icon::StatusTodo => "status-todo.svg",
            Icon::StatusProgress => "status-progress.svg",
            Icon::StatusBlocked => "status-blocked.svg",
            Icon::StatusDone => "status-done.svg",
            Icon::StatusCancelled => "status-cancelled.svg",
            Icon::PriorityUrgent => "priority-urgent.svg",
            Icon::PriorityHigh => "priority-high.svg",
            Icon::PriorityMedium => "priority-medium.svg",
            Icon::PriorityLow => "priority-low.svg",
            Icon::PriorityNone => "priority-none.svg",
            Icon::Dot => "dot.svg",
            Icon::Link => "link.svg",
            Icon::Tag => "tag.svg",
            Icon::Filter => "filter.svg",
            Icon::Board => "board.svg",
            Icon::List => "list.svg",
            Icon::Send => "send.svg",
            Icon::OpenAi => "openai.svg",
            Icon::Agents => "agents.svg",
            Icon::ArrowRight => "arrowRight.svg",
            Icon::ArrowUpRight => "arrowUpRight.svg",
            Icon::ChevronLeft => "chevronLeft.svg",
            Icon::ChevronRight => "chevronRight.svg",
            Icon::Close => "close.svg",
            Icon::EveeOutline => "evee-outline.svg",
            Icon::Panel => "panel.svg",
            Icon::Plus => "plus.svg",
            Icon::Search => "search.svg",
            Icon::Settings => "settings.svg",
            Icon::Spark => "spark.svg",
            Icon::Tasks => "tickets.svg",
            Icon::Terminal => "terminal.svg",
        }
    }

    /// The embedded SVG.
    pub fn bytes(self) -> &'static [u8] {
        match self {
            Icon::Refresh => include_bytes!("../../assets/icons/refresh.svg"),
            Icon::Temporal => include_bytes!("../../assets/icons/temporal.svg"),
            Icon::Check => include_bytes!("../../assets/icons/check.svg"),
            Icon::ChevronDown => include_bytes!("../../assets/icons/chevronDown.svg"),
            Icon::ChevronUpDown => include_bytes!("../../assets/icons/chevronUpDown.svg"),
            Icon::More => include_bytes!("../../assets/icons/more.svg"),
            Icon::User => include_bytes!("../../assets/icons/user.svg"),
            Icon::Users => include_bytes!("../../assets/icons/users.svg"),
            Icon::Help => include_bytes!("../../assets/icons/help.svg"),
            Icon::Feedback => include_bytes!("../../assets/icons/feedback.svg"),
            Icon::Download => include_bytes!("../../assets/icons/download.svg"),
            Icon::Inbox => include_bytes!("../../assets/icons/inbox.svg"),
            Icon::Warning => include_bytes!("../../assets/icons/warning.svg"),
            Icon::Info => include_bytes!("../../assets/icons/info.svg"),
            Icon::Copy => include_bytes!("../../assets/icons/copy.svg"),
            Icon::Trash => include_bytes!("../../assets/icons/trash.svg"),
            Icon::Edit => include_bytes!("../../assets/icons/edit.svg"),
            Icon::Play => include_bytes!("../../assets/icons/play.svg"),
            Icon::Pause => include_bytes!("../../assets/icons/pause.svg"),
            Icon::Stop => include_bytes!("../../assets/icons/stop.svg"),
            Icon::History => include_bytes!("../../assets/icons/history.svg"),
            Icon::Repeat => include_bytes!("../../assets/icons/repeat.svg"),
            Icon::Command => include_bytes!("../../assets/icons/command.svg"),
            Icon::StatusBacklog => include_bytes!("../../assets/icons/status-backlog.svg"),
            Icon::StatusTodo => include_bytes!("../../assets/icons/status-todo.svg"),
            Icon::StatusProgress => include_bytes!("../../assets/icons/status-progress.svg"),
            Icon::StatusBlocked => include_bytes!("../../assets/icons/status-blocked.svg"),
            Icon::StatusDone => include_bytes!("../../assets/icons/status-done.svg"),
            Icon::StatusCancelled => include_bytes!("../../assets/icons/status-cancelled.svg"),
            Icon::PriorityUrgent => include_bytes!("../../assets/icons/priority-urgent.svg"),
            Icon::PriorityHigh => include_bytes!("../../assets/icons/priority-high.svg"),
            Icon::PriorityMedium => include_bytes!("../../assets/icons/priority-medium.svg"),
            Icon::PriorityLow => include_bytes!("../../assets/icons/priority-low.svg"),
            Icon::PriorityNone => include_bytes!("../../assets/icons/priority-none.svg"),
            Icon::Dot => include_bytes!("../../assets/icons/dot.svg"),
            Icon::Link => include_bytes!("../../assets/icons/link.svg"),
            Icon::Tag => include_bytes!("../../assets/icons/tag.svg"),
            Icon::Filter => include_bytes!("../../assets/icons/filter.svg"),
            Icon::Board => include_bytes!("../../assets/icons/board.svg"),
            Icon::List => include_bytes!("../../assets/icons/list.svg"),
            Icon::Send => include_bytes!("../../assets/send.svg"),
            Icon::OpenAi => include_bytes!("../../assets/openai.svg"),
            Icon::Agents => include_bytes!("../../assets/agents.svg"),
            Icon::ArrowRight => include_bytes!("../../assets/arrowRight.svg"),
            Icon::ArrowUpRight => include_bytes!("../../assets/arrowUpRight.svg"),
            Icon::ChevronLeft => include_bytes!("../../assets/chevronLeft.svg"),
            Icon::ChevronRight => include_bytes!("../../assets/chevronRight.svg"),
            Icon::Close => include_bytes!("../../assets/close.svg"),
            Icon::EveeOutline => include_bytes!("../../assets/evee-outline.svg"),
            Icon::Panel => include_bytes!("../../assets/panel.svg"),
            Icon::Plus => include_bytes!("../../assets/plus.svg"),
            Icon::Search => include_bytes!("../../assets/search.svg"),
            Icon::Settings => include_bytes!("../../assets/settings.svg"),
            Icon::Spark => include_bytes!("../../assets/spark.svg"),
            Icon::Tasks => include_bytes!("../../assets/tickets.svg"),
            Icon::Terminal => include_bytes!("../../assets/terminal.svg"),
        }
    }

    pub fn from_path(path: &str) -> Option<Icon> {
        Self::ALL.into_iter().find(|icon| icon.path() == path)
    }

    /// Add and create actions put their `+` after the label; every other icon
    /// leads it. This is the one place that rule lives.
    pub fn trails_label(self) -> bool {
        self == Icon::Plus
    }

    /// The path of the heavier-stroke variant drawn for a selected navigation item.
    pub fn selected_path(self) -> SharedString {
        format!("selected/{}", self.path()).into()
    }
}

#[cfg(test)]
mod tests {
    use super::Icon;

    #[test]
    fn every_icon_round_trips_through_its_path() {
        for icon in Icon::ALL {
            assert_eq!(Icon::from_path(icon.path()), Some(icon));
            assert!(!icon.bytes().is_empty());
        }
        assert_eq!(Icon::from_path("missing.svg"), None);
        assert!(Icon::Plus.trails_label() && !Icon::Search.trails_label());
    }
}
