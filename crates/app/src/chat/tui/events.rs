use super::super::live_surface::CliChatLiveSurfaceSnapshot;

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum UiEvent {
    ComposerInput(char),
    Backspace,
    AppendUserMessage(String),
    UpdateAssistantStream(String),
    FinalizeAssistantMessage(String),
    UpdateLiveSurface(CliChatLiveSurfaceSnapshot),
    ExitRequested,
}
