use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::catalog;
use super::external_skills;
use super::runtime_config;
use super::tool_search::{SearchableToolEntry, runtime_discoverable_tool_entries};
use super::tool_surface;
use super::{ToolView, effective_runtime_visible_tool_view, runtime_tool_view_for_runtime_config};

#[derive(Debug, Clone)]
pub struct ToolRegistryEntry {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoverableToolSurfaceSummary {
    #[serde(default)]
    pub visible_direct_tools: Vec<String>,
    pub hidden_tool_count: usize,
    #[serde(default)]
    pub hidden_tags: Vec<String>,
    #[serde(default)]
    pub hidden_surfaces: Vec<super::ToolSurfaceState>,
}

pub fn tool_registry() -> Vec<ToolRegistryEntry> {
    tool_registry_with_config(Some(runtime_config::get_tool_runtime_config()))
}

pub fn tool_registry_with_config(
    config: Option<&runtime_config::ToolRuntimeConfig>,
) -> Vec<ToolRegistryEntry> {
    let default_runtime_config;
    let config = match config {
        Some(config) => config,
        None => {
            default_runtime_config = runtime_config::ToolRuntimeConfig::default();
            &default_runtime_config
        }
    };

    let discoverable_entries = runtime_discoverable_tool_entries(config, None, false);
    let mut entries = Vec::new();

    for entry in discoverable_entries {
        let registry_entry = ToolRegistryEntry {
            name: entry.canonical_name,
            description: entry.summary,
        };
        entries.push(registry_entry);
    }

    entries.sort_by(|left, right| left.name.cmp(&right.name));
    entries
}

pub fn capability_snapshot() -> String {
    capability_snapshot_with_config(runtime_config::get_tool_runtime_config())
}

pub fn capability_snapshot_with_config(config: &runtime_config::ToolRuntimeConfig) -> String {
    capability_snapshot_for_view_with_config(&runtime_tool_view_for_runtime_config(config), config)
}

pub fn capability_snapshot_for_view(view: &ToolView) -> String {
    capability_snapshot_for_view_with_config(view, runtime_config::get_tool_runtime_config())
}

pub(crate) fn capability_snapshot_for_view_with_config(
    view: &ToolView,
    config: &runtime_config::ToolRuntimeConfig,
) -> String {
    let mut lines = vec![
        "[tool_discovery_runtime]".to_owned(),
        "Available tools:".to_owned(),
    ];

    let visible_direct_states = tool_surface::visible_direct_tool_states_for_view(view);
    let visible_direct_lines = render_visible_direct_tool_lines(visible_direct_states.as_slice());
    lines.extend(visible_direct_lines);

    let gateway_entries = catalog::provider_exposed_tool_catalog();
    let gateway_entries = gateway_entries
        .into_iter()
        .filter(|entry| entry.is_gateway())
        .collect::<Vec<_>>();
    for entry in gateway_entries {
        let line = format!("- {}: {}", entry.canonical_name, entry.summary);
        lines.push(line);
    }

    let discoverable_summary =
        runtime_discoverable_tool_surface_summary_with_config(config, Some(view));
    let hidden_tool_count = discoverable_summary.hidden_tool_count;

    if hidden_tool_count == 0 {
        lines.push(
            "No additional specialized tools are currently available through tool.search."
                .to_owned(),
        );
    } else {
        let hidden_count_line = format!(
            "Additional specialized tools available through tool.search: {hidden_tool_count}."
        );
        lines.push(hidden_count_line);

        let hidden_surface_lines =
            render_hidden_tool_surface_lines(discoverable_summary.hidden_surfaces.as_slice());
        lines.extend(hidden_surface_lines);

        let hidden_tag_line = hidden_tool_tag_line(discoverable_summary.hidden_tags.as_slice());
        if let Some(hidden_tag_line) = hidden_tag_line {
            lines.push(hidden_tag_line);
        }
    }

    lines.push("Guidelines:".to_owned());
    lines.extend(render_active_tool_guideline_lines(
        visible_direct_states.as_slice(),
        discoverable_summary.hidden_surfaces.as_slice(),
    ));
    if let Some(skill_catalog_section) =
        external_skills::model_skill_catalog_section_with_config(config)
    {
        lines.push(skill_catalog_section);
    }
    lines.join("\n")
}

fn render_visible_direct_tool_lines(states: &[super::ToolSurfaceState]) -> Vec<String> {
    let mut lines = Vec::new();

    for state in states {
        let line = format!(
            "- {}: {} {}",
            state.surface_id, state.prompt_snippet, state.usage_guidance
        );
        lines.push(line);
    }

    lines
}

fn render_active_tool_guideline_lines(
    visible_direct_states: &[super::ToolSurfaceState],
    hidden_surfaces: &[super::ToolSurfaceState],
) -> Vec<String> {
    let mut lines = vec![
        "- Prefer a direct tool when one clearly fits.".to_owned(),
        "- Use tool.search only when you need a specialized capability that is not already direct.".to_owned(),
        "- Keep tool.search queries short and capability-focused.".to_owned(),
        "- Use tool.invoke only with a fresh lease returned by tool.search.".to_owned(),
        "- If the user wants different permissions or guardrails, edit the relevant config or prompt files instead of treating the runtime as fixed.".to_owned(),
    ];
    let mut seen = lines.iter().cloned().collect::<BTreeSet<_>>();

    for surface in visible_direct_states.iter().chain(hidden_surfaces.iter()) {
        let Some(guidelines) =
            tool_surface::tool_surface_prompt_guidelines_for_id(surface.surface_id.as_str())
        else {
            continue;
        };
        for guideline in guidelines {
            let line = format!("- {guideline}");
            let inserted = seen.insert(line.clone());
            if inserted {
                lines.push(line);
            }
        }
    }

    lines
}

fn render_hidden_tool_surface_lines(surfaces: &[super::ToolSurfaceState]) -> Vec<String> {
    surfaces
        .iter()
        .map(super::ToolSurfaceState::render_prompt_line)
        .collect()
}

pub fn runtime_discoverable_tool_surface_summary_with_config(
    config: &runtime_config::ToolRuntimeConfig,
    visible_tool_view: Option<&ToolView>,
) -> DiscoverableToolSurfaceSummary {
    let effective_view = effective_runtime_visible_tool_view(config, visible_tool_view);
    let discoverable_entries =
        runtime_discoverable_tool_entries(config, Some(&effective_view), true);
    let direct_states = tool_surface::visible_direct_tool_states_for_view(&effective_view);
    summarize_discoverable_tool_surface(discoverable_entries.as_slice(), direct_states)
}

fn summarize_discoverable_tool_surface(
    discoverable_entries: &[SearchableToolEntry],
    direct_states: Vec<super::ToolSurfaceState>,
) -> DiscoverableToolSurfaceSummary {
    let visible_direct_tools = direct_states
        .into_iter()
        .map(|state| state.surface_id)
        .collect::<Vec<_>>();
    let hidden_surfaces = tool_surface::active_discoverable_tool_surface_states(
        discoverable_entries
            .iter()
            .map(|entry| entry.tool_id.as_str()),
    );

    DiscoverableToolSurfaceSummary {
        visible_direct_tools,
        hidden_tool_count: discoverable_entries.len(),
        hidden_tags: summarize_hidden_tool_tags(discoverable_entries),
        hidden_surfaces,
    }
}

fn hidden_tool_tag_line(hidden_tags: &[String]) -> Option<String> {
    if hidden_tags.is_empty() {
        return None;
    }

    let joined_tags = hidden_tags.join(", ");
    Some(format!(
        "Hidden specialized tool tags currently discoverable: {joined_tags}."
    ))
}

fn summarize_hidden_tool_tags(entries: &[SearchableToolEntry]) -> Vec<String> {
    const IGNORED_TAGS: &[&str] = &["core", "discover", "search", "dispatch", "invoke"];
    const MAX_DISCOVERABLE_CAPABILITY_TAGS: usize = 8;

    let mut tag_counts = BTreeMap::<String, usize>::new();

    for entry in entries {
        for tag in &entry.tags {
            let normalized_tag = tag.trim();
            if normalized_tag.is_empty() {
                continue;
            }

            let ignored_tag = IGNORED_TAGS.contains(&normalized_tag);
            if ignored_tag {
                continue;
            }

            let count_entry = tag_counts.entry(normalized_tag.to_owned()).or_insert(0);
            *count_entry += 1;
        }
    }

    let mut ranked_tags = tag_counts.into_iter().collect::<Vec<_>>();
    ranked_tags.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));

    ranked_tags
        .into_iter()
        .take(MAX_DISCOVERABLE_CAPABILITY_TAGS)
        .map(|(tag, _count)| tag)
        .collect()
}
