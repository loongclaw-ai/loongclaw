#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CliChatUiMode {
    #[default]
    Text,
    Tui,
}
