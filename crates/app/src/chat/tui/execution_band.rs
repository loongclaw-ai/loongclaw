use super::super::live_surface::CliChatLiveSurfaceSnapshot;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ExecutionBandSummary {
    pub(crate) running_count: usize,
    pub(crate) pending_approval_count: usize,
    pub(crate) latest_result: Option<String>,
    pub(crate) background_count: usize,
}

pub(crate) fn project_execution_band_summary(
    snapshot: &CliChatLiveSurfaceSnapshot,
) -> ExecutionBandSummary {
    let running_count = snapshot
        .tool_activity_lines
        .iter()
        .filter(|line| line.starts_with("[running]"))
        .count();
    let latest_result = snapshot
        .tool_activity_lines
        .iter()
        .rev()
        .find(|line| !line.starts_with("args:") && !line.starts_with("[running]"))
        .cloned();

    ExecutionBandSummary {
        running_count,
        pending_approval_count: 0,
        latest_result,
        background_count: 0,
    }
}

pub(crate) fn render_execution_band_summary(summary: &ExecutionBandSummary) -> String {
    let latest_result = summary.latest_result.as_deref().unwrap_or("none");
    format!(
        "running {} | approvals {} | background {} | latest {}",
        summary.running_count,
        summary.pending_approval_count,
        summary.background_count,
        latest_result
    )
}
