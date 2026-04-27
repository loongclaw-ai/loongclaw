use super::{
    SearchDocument, SearchableToolEntry, append_unique_sentence, build_argument_fragments,
};

pub(super) fn enrich_searchable_entries_for_skill_hints(
    entries: Vec<SearchableToolEntry>,
    skill_hints: &[super::super::external_skills::SkillDiscoveryToolHint],
    exact_skill_hint: Option<&super::super::external_skills::SkillDiscoveryToolHint>,
) -> Vec<SearchableToolEntry> {
    entries
        .into_iter()
        .map(|mut entry| {
            if entry.tool_id == "skills" {
                enrich_skills_surface_entry(&mut entry, skill_hints, exact_skill_hint);
            }
            entry
        })
        .collect()
}

pub(super) fn enrich_skills_surface_entry(
    entry: &mut SearchableToolEntry,
    skill_hints: &[super::super::external_skills::SkillDiscoveryToolHint],
    exact_skill_hint: Option<&super::super::external_skills::SkillDiscoveryToolHint>,
) {
    let mut matched_skills = Vec::new();
    if let Some(skill_hint) = exact_skill_hint {
        matched_skills.push(skill_hint.clone());
    }
    for skill_hint in skill_hints {
        if matched_skills.iter().any(|existing| {
            existing
                .skill_id
                .eq_ignore_ascii_case(skill_hint.skill_id.as_str())
        }) {
            continue;
        }
        matched_skills.push(skill_hint.clone());
    }
    if matched_skills.is_empty() {
        return;
    }

    let matched_skill_labels = matched_skills
        .iter()
        .map(|skill| {
            let display_name = skill.display_name.trim();
            if display_name.is_empty() || display_name.eq_ignore_ascii_case(skill.skill_id.as_str())
            {
                skill.skill_id.clone()
            } else {
                format!("{} ({display_name})", skill.skill_id)
            }
        })
        .collect::<Vec<_>>();
    let matched_skill_summary = format!(
        "Matching installed skills: {}.",
        matched_skill_labels.join(", ")
    );
    entry.summary = append_unique_sentence(entry.summary.as_str(), matched_skill_summary.as_str());
    entry.search_hint =
        append_unique_sentence(entry.search_hint.as_str(), matched_skill_summary.as_str());

    let preferred_skill = exact_skill_hint.or_else(|| matched_skills.first());
    let mut usage_guidance = entry
        .usage_guidance
        .clone()
        .unwrap_or_else(|| "Use this when the task is about capability expansion.".to_owned());
    if let Some(skill) = preferred_skill {
        let invoke_example = format!(
            "{{\"operation\":\"run\",\"skill_id\":\"{}\"}}",
            skill.skill_id
        );
        let inspect_example = format!(
            "{{\"operation\":\"inspect\",\"skill_id\":\"{}\"}}",
            skill.skill_id
        );
        let guidance = format!(
            "To load a specific installed skill through this surface, call tool.invoke with the lease from this card and arguments {invoke_example}. Use {inspect_example} to inspect metadata, or {{\"operation\":\"list\"}} to enumerate installed skills."
        );
        usage_guidance = append_unique_sentence(usage_guidance.as_str(), guidance.as_str());
    }
    entry.usage_guidance = Some(usage_guidance);

    let mut tags = entry.tags.clone();
    for skill in &matched_skills {
        if !tags
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(skill.skill_id.as_str()))
        {
            tags.push(skill.skill_id.clone());
        }
        let display_name = skill.display_name.trim();
        if !display_name.is_empty()
            && !tags
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(display_name))
        {
            tags.push(display_name.to_owned());
        }
    }
    entry.tags = tags;

    let mut name_fragments = vec![entry.canonical_name.clone(), entry.tool_id.clone()];
    let mut summary_fragments = vec![entry.summary.clone(), entry.search_hint.clone()];
    let argument_fragments = build_argument_fragments(
        entry.argument_hint.as_str(),
        &entry.required_fields,
        &entry.required_field_groups,
    );
    let schema_fragments = vec![
        entry.required_fields.join(" "),
        entry
            .required_field_groups
            .iter()
            .map(|group| group.join(" "))
            .collect::<Vec<_>>()
            .join(" "),
    ]
    .into_iter()
    .filter(|fragment| !fragment.trim().is_empty())
    .collect::<Vec<_>>();
    let mut tag_fragments = entry.tags.clone();

    for skill in &matched_skills {
        name_fragments.push(skill.skill_id.clone());
        let display_name = skill.display_name.trim();
        if !display_name.is_empty() {
            name_fragments.push(display_name.to_owned());
        }
        let skill_summary = skill.summary.trim();
        if !skill_summary.is_empty() {
            summary_fragments.push(skill_summary.to_owned());
        }
        tag_fragments.push(skill.skill_id.clone());
    }

    entry.search_document = SearchDocument::new(
        name_fragments,
        summary_fragments,
        argument_fragments,
        schema_fragments,
        tag_fragments,
    );
}
