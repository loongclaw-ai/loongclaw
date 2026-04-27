use std::borrow::Cow;
use std::collections::BTreeSet;

use loong_contracts::{Capability, ToolCoreOutcome, ToolCoreRequest};
use serde_json::Value;
use serde_json::json;

use super::catalog::{ToolDescriptor, ToolView};
use super::runtime_config;
use super::{
    LOONG_INTERNAL_TOOL_SEARCH_KEY, LOONG_INTERNAL_TOOL_SEARCH_VISIBLE_TOOL_IDS_KEY,
    TOOL_SEARCH_GRANTED_CAPABILITIES_FIELD, canonical_tool_name, issue_tool_lease, memory_tools,
};

#[path = "tool_search_entry.rs"]
mod entry;
#[path = "tool_search_query_support.rs"]
mod query_support;
#[path = "tool_search_rank.rs"]
mod rank;
#[path = "tool_search_skill.rs"]
mod skill;
#[path = "tool_search_view.rs"]
mod view;
#[cfg(test)]
use entry::schema_required_field_groups;
pub(crate) use entry::{
    SearchableToolEntry, build_argument_fragments, collapse_hidden_surface_search_entries,
    searchable_entry_from_manual_definition, searchable_entry_from_provider_definition,
};
#[cfg(test)]
use query_support::*;
pub(crate) use rank::rank_searchable_entries;
use skill::enrich_searchable_entries_for_skill_hints;
use skill::enrich_skills_surface_entry;
use view::searchable_entry_from_descriptor_for_view;
pub(crate) use view::tool_id_visible_in_view;
pub(crate) use view::{
    provider_visible_collapsible_hidden_surface_ids, runtime_discoverable_tool_entries,
    runtime_tool_search_entries,
};

#[derive(Debug, Clone)]
pub(super) struct RankedSearchableToolEntry {
    pub(super) entry: SearchableToolEntry,
    pub(super) why: Vec<String>,
}

#[derive(Debug, Clone)]
pub(super) struct ToolSearchRanking {
    pub(super) results: Vec<RankedSearchableToolEntry>,
    pub(super) diagnostics_reason: Option<&'static str>,
}

pub(super) fn execute_tool_search_tool_with_config(
    request: ToolCoreRequest,
    config: &runtime_config::ToolRuntimeConfig,
) -> Result<ToolCoreOutcome, String> {
    let payload = request
        .payload
        .as_object()
        .ok_or_else(|| "tool.search payload must be an object".to_owned())?;
    let query = tool_search_query_from_payload(payload).map(Cow::into_owned);
    let requested_exact_tool_id = payload
        .get("exact_tool_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let exact_tool_id = requested_exact_tool_id
        .as_deref()
        .map(canonical_tool_name)
        .map(str::to_owned);

    let limit = payload
        .get("limit")
        .and_then(Value::as_u64)
        .map(|value| value.clamp(1, 8) as usize)
        .unwrap_or(5);
    let granted_capabilities = payload
        .get(TOOL_SEARCH_GRANTED_CAPABILITIES_FIELD)
        .cloned()
        .and_then(|value| serde_json::from_value::<BTreeSet<Capability>>(value).ok());
    let visible_tool_view = search_tool_view_from_payload(payload, config);
    let exact_match_entries = runtime_tool_search_entries(config, Some(&visible_tool_view), false)
        .into_iter()
        .filter(|entry| {
            tool_search_entry_is_capability_usable(
                entry.canonical_name.as_str(),
                granted_capabilities.as_ref(),
            )
        })
        .collect::<Vec<_>>();
    let collapsible_surface_ids =
        provider_visible_collapsible_hidden_surface_ids(config, &visible_tool_view);
    let skill_query_hints = query
        .as_deref()
        .map(|value| super::external_skills::ranked_model_visible_skill_hints(config, value, 3))
        .transpose()?
        .unwrap_or_default();
    let exact_skill_hint = requested_exact_tool_id
        .as_deref()
        .map(|value| super::external_skills::exact_model_visible_skill_hint(config, value))
        .transpose()?
        .flatten();
    let searchable_entries = collapse_hidden_surface_search_entries(
        exact_match_entries.clone(),
        &collapsible_surface_ids,
    );
    let searchable_entries = enrich_searchable_entries_for_skill_hints(
        searchable_entries,
        skill_query_hints.as_slice(),
        exact_skill_hint.as_ref(),
    );
    let exact_match_entry = exact_tool_id
        .as_ref()
        .and_then(|exact_tool_id| {
            let direct_tool_id = super::direct_tool_name_for_hidden_tool(exact_tool_id);
            let direct_tool_id = direct_tool_id.map(str::to_owned);

            searchable_entries
                .iter()
                .find(|entry| {
                    let canonical_match = entry.canonical_name == *exact_tool_id;
                    let tool_id_match = entry.tool_id == *exact_tool_id;
                    let direct_match = direct_tool_id.as_ref().is_some_and(|direct_tool_id| {
                        entry.canonical_name == *direct_tool_id || entry.tool_id == *direct_tool_id
                    });
                    canonical_match || tool_id_match || direct_match
                })
                .cloned()
                .or_else(|| {
                    exact_match_entries
                        .iter()
                        .find(|entry| {
                            let canonical_match = entry.canonical_name == *exact_tool_id;
                            let tool_id_match = entry.tool_id == *exact_tool_id;
                            let direct_match =
                                direct_tool_id.as_ref().is_some_and(|direct_tool_id| {
                                    entry.canonical_name == *direct_tool_id
                                });
                            canonical_match || tool_id_match || direct_match
                        })
                        .cloned()
                })
        })
        .or_else(|| {
            exact_skill_hint.as_ref().and_then(|hint| {
                searchable_entries
                    .iter()
                    .find(|entry| entry.tool_id == "skills")
                    .cloned()
                    .map(|mut entry| {
                        enrich_skills_surface_entry(
                            &mut entry,
                            std::slice::from_ref(hint),
                            Some(hint),
                        );
                        entry
                    })
            })
        });
    let exact_match_found = exact_match_entry.is_some();
    let mut diagnostics_reason = None;
    let results: Vec<Value> = if let Some(entry) = exact_match_entry {
        let why = Vec::new();
        let entry_json = tool_search_result_entry_json(&entry, why, payload)?;
        vec![entry_json]
    } else if let Some(query) = query.as_deref() {
        let ranking = rank_searchable_entries(searchable_entries, query, limit);
        diagnostics_reason = ranking.diagnostics_reason;

        ranking
            .results
            .into_iter()
            .map(|ranked_entry| {
                let RankedSearchableToolEntry { entry, why } = ranked_entry;

                tool_search_result_entry_json(&entry, why, payload)
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        let ranking = rank_searchable_entries(searchable_entries, "", limit);
        diagnostics_reason = ranking.diagnostics_reason;

        ranking
            .results
            .into_iter()
            .map(|ranked_entry| {
                let RankedSearchableToolEntry { entry, why } = ranked_entry;

                tool_search_result_entry_json(&entry, why, payload)
            })
            .collect::<Result<Vec<_>, _>>()?
    };
    let diagnostics = tool_search_diagnostics_json(
        requested_exact_tool_id.as_deref(),
        exact_match_found,
        query.as_deref(),
        diagnostics_reason,
    );
    let response_exact_tool_id = if exact_match_found {
        exact_tool_id
    } else {
        requested_exact_tool_id
    };

    Ok(ToolCoreOutcome {
        status: "ok".to_owned(),
        payload: json!({
            "adapter": "core-tools",
            "tool_name": request.tool_name,
            "query": query,
            "exact_tool_id": response_exact_tool_id,
            "returned": results.len(),
            "results": results,
            "diagnostics": diagnostics,
        }),
    })
}

fn tool_search_result_entry_json(
    entry: &SearchableToolEntry,
    why: Vec<String>,
    payload: &serde_json::Map<String, Value>,
) -> Result<Value, String> {
    let mut result = serde_json::Map::from_iter([
        ("tool_id".to_owned(), json!(entry.tool_id)),
        ("summary".to_owned(), json!(entry.summary)),
        ("search_hint".to_owned(), json!(entry.search_hint)),
        ("argument_hint".to_owned(), json!(entry.argument_hint)),
        ("required_fields".to_owned(), json!(entry.required_fields)),
        (
            "required_field_groups".to_owned(),
            json!(entry.required_field_groups),
        ),
        ("schema_preview".to_owned(), json!(entry.schema_preview)),
        ("tags".to_owned(), json!(entry.tags)),
        ("why".to_owned(), json!(why)),
    ]);
    if entry.requires_lease {
        let lease = issue_tool_lease(entry.canonical_name.as_str(), payload)?;
        result.insert("lease".to_owned(), json!(lease));
    }
    if let Some(surface_id) = entry.surface_id.as_deref() {
        result.insert(
            "surface_id".to_owned(),
            Value::String(surface_id.to_owned()),
        );
    }
    if let Some(usage_guidance) = entry.usage_guidance.as_deref() {
        result.insert(
            "usage_guidance".to_owned(),
            Value::String(usage_guidance.to_owned()),
        );
    }
    Ok(Value::Object(result))
}

fn tool_search_diagnostics_json(
    requested_exact_tool_id: Option<&str>,
    exact_match_found: bool,
    query: Option<&str>,
    diagnostics_reason: Option<&str>,
) -> Value {
    if let Some(requested_exact_tool_id) = requested_exact_tool_id {
        if exact_match_found {
            return Value::Null;
        }

        return json!({
            "reason": "exact_tool_id_not_visible",
            "requested_tool_id": requested_exact_tool_id,
        });
    }

    if let Some(reason) = diagnostics_reason {
        let diagnostics_query = query.unwrap_or_default();

        return json!({
            "reason": reason,
            "query": diagnostics_query,
        });
    }

    Value::Null
}

fn append_unique_sentence(base: &str, addition: &str) -> String {
    let trimmed_addition = addition.trim();
    if trimmed_addition.is_empty() {
        return base.trim().to_owned();
    }

    let trimmed_base = base.trim();
    if trimmed_base.is_empty() {
        return trimmed_addition.to_owned();
    }

    if trimmed_base.contains(trimmed_addition) {
        return trimmed_base.to_owned();
    }

    format!("{trimmed_base} {trimmed_addition}")
}

fn tool_search_query_from_payload(
    payload: &serde_json::Map<String, Value>,
) -> Option<Cow<'_, str>> {
    const QUERY_KEYS: &[&str] = &["query", "input", "text", "prompt", "keyword", "keywords"];

    for key in QUERY_KEYS {
        let Some(value) = payload.get(*key) else {
            continue;
        };

        if let Some(query) = tool_search_query_from_value(value) {
            return Some(query);
        }
    }

    None
}

fn tool_search_query_from_value(value: &Value) -> Option<Cow<'_, str>> {
    let string_value = value.as_str();
    if let Some(string_value) = string_value {
        let trimmed_value = string_value.trim();
        if !trimmed_value.is_empty() {
            return Some(Cow::Borrowed(trimmed_value));
        }
    }

    let values = value.as_array()?;
    let joined_value = join_tool_search_query_values(values);
    if joined_value.is_empty() {
        return None;
    }

    Some(Cow::Owned(joined_value))
}

fn join_tool_search_query_values(values: &[Value]) -> String {
    let mut query_parts = Vec::new();

    for value in values {
        let query_part = tool_search_query_part(value);
        if query_part.is_empty() {
            continue;
        }

        query_parts.push(query_part);
    }

    query_parts.join(" ")
}

fn tool_search_query_part(value: &Value) -> String {
    let string_value = value.as_str();
    if let Some(string_value) = string_value {
        return string_value.trim().to_owned();
    }

    value.to_string()
}

pub(super) fn tool_search_entry_is_runtime_usable(
    tool_name: &str,
    config: &runtime_config::ToolRuntimeConfig,
) -> bool {
    match tool_name {
        "shell.exec" => {
            !config.shell_allow.is_empty()
                || matches!(
                    config.shell_default_mode,
                    crate::tools::shell_policy_ext::ShellPolicyDefault::Allow
                )
        }
        "bash.exec" => config.bash_exec.is_discoverable(),
        "external_skills.fetch"
        | "external_skills.install"
        | "external_skills.inspect"
        | "external_skills.invoke"
        | "external_skills.list"
        | "external_skills.remove" => config.external_skills.enabled,
        #[cfg(feature = "tool-file")]
        "memory_search" => memory_tools::memory_corpus_available(config),
        #[cfg(feature = "tool-file")]
        "memory_get" => memory_tools::workspace_memory_corpus_available(config),
        _ => true,
    }
}

pub(super) fn tool_search_entry_is_capability_usable(
    tool_name: &str,
    granted_capabilities: Option<&BTreeSet<Capability>>,
) -> bool {
    let Some(granted_capabilities) = granted_capabilities else {
        return true;
    };
    let required = super::required_capabilities_for_tool_name_and_payload(tool_name, &json!({}));
    required
        .iter()
        .all(|capability| granted_capabilities.contains(capability))
}

pub(super) fn search_tool_view_from_payload(
    payload: &serde_json::Map<String, Value>,
    config: &runtime_config::ToolRuntimeConfig,
) -> ToolView {
    let payload_value = Value::Object(payload.clone());
    let visible_tool_names = if super::trusted_internal_tool_payload_enabled() {
        super::trusted_internal_tool_context_from_payload(&payload_value)
            .and_then(|body| body.get(LOONG_INTERNAL_TOOL_SEARCH_KEY))
            .and_then(|body| body.get(LOONG_INTERNAL_TOOL_SEARCH_VISIBLE_TOOL_IDS_KEY))
            .and_then(Value::as_array)
            .map(|tool_names| {
                tool_names
                    .iter()
                    .filter_map(Value::as_str)
                    .map(canonical_tool_name)
                    .collect::<Vec<_>>()
            })
    } else {
        None
    };

    match visible_tool_names {
        Some(visible_tool_names) => ToolView::from_tool_names(visible_tool_names),
        None => super::full_runtime_tool_view_for_runtime_config(config),
    }
}

pub(super) fn searchable_entry_from_descriptor(descriptor: &ToolDescriptor) -> SearchableToolEntry {
    searchable_entry_from_descriptor_for_view(descriptor, None)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_search_text_keeps_ascii_queries_stable() {
        let normalized = normalize_search_text("Find README.md");
        assert_eq!(normalized, "find readme.md");
    }

    #[test]
    fn english_concepts_extract_from_prompt_style_queries() {
        let fragments = vec!["install skill".to_owned()];
        let signal = SearchSignalSet::from_fragments(&fragments);
        let (concepts, categories) = extract_concepts_and_categories(&signal);

        assert!(concepts.contains("install"));
        assert!(concepts.contains("skill"));
        assert!(categories.contains("extension"));
        assert!(categories.contains("mutation"));
    }

    #[test]
    fn structural_query_hints_detect_file_references() {
        let query = SearchQuery::new("read note.md");
        assert!(query.concepts.contains("file"));
        assert!(query.categories.contains("workspace"));
    }

    #[test]
    fn schema_required_field_groups_merge_root_and_branch_requirements() {
        let schema = serde_json::json!({
            "type": "object",
            "required": ["url"],
            "properties": {
                "url": {"type": "string"},
                "content": {"type": "string"},
                "content_path": {"type": "string"}
            },
            "anyOf": [
                {"required": ["content"]},
                {}
            ]
        });
        let required_field_groups = schema_required_field_groups(&schema);

        assert_eq!(
            required_field_groups,
            vec![
                vec!["url".to_owned(), "content".to_owned()],
                vec!["url".to_owned()],
            ]
        );
    }

    #[test]
    fn structural_query_hints_do_not_treat_lone_domains_as_files() {
        let query = SearchQuery::new("example.com");

        assert!(!query.concepts.contains("file"));
        assert!(!query.categories.contains("workspace"));
    }

    #[test]
    fn structural_query_hints_do_not_treat_domain_paths_as_files() {
        let query = SearchQuery::new("example.com/path");

        assert!(!query.concepts.contains("file"));
        assert!(!query.categories.contains("workspace"));
    }

    #[test]
    fn structural_query_hints_do_not_treat_version_tokens_as_files() {
        let version_query = SearchQuery::new("gpt-4.1");
        let numeric_query = SearchQuery::new("3.14");

        assert!(!version_query.concepts.contains("file"));
        assert!(!numeric_query.concepts.contains("file"));
    }

    #[test]
    fn structural_query_hints_do_not_treat_generic_tree_queries_as_directories() {
        let query = SearchQuery::new("binary tree traversal");

        assert!(!query.concepts.contains("directory"));
        assert!(!query.categories.contains("workspace"));
    }

    #[test]
    fn single_dotted_identifier_queries_keep_search_signals() {
        let query = SearchQuery::new("bash.exec");

        assert!(query.signal.contains_term("bash"));
        assert!(query.signal.contains_term("exec"));
        assert!(query.signal.normalized_text.contains("bash.exec"));
        assert!(!query.concepts.contains("file"));
    }
}
