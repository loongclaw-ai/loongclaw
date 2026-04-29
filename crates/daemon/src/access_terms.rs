pub(crate) const QUERY_SEARCH_PROVIDER_LABEL: &str = "query search provider";
pub(crate) const QUERY_SEARCH_CREDENTIAL_LABEL: &str = "query search credential";
pub(crate) const QUERY_SEARCH_CREDENTIAL_SOURCE_LABEL: &str = "query search credential source";
pub(crate) const CHOOSE_QUERY_SEARCH_TITLE: &str = "choose query search";
pub(crate) const CHOOSE_QUERY_SEARCH_PROVIDER_TITLE: &str = "choose query search provider";
pub(crate) const CHOOSE_QUERY_SEARCH_CREDENTIAL_TITLE: &str = "choose query search credential";
pub(crate) const ACCESS_BOUNDARY_LABEL: &str = "access boundary";

pub(crate) fn query_search_provider_selection_index_error(idx: usize) -> String {
    format!("{QUERY_SEARCH_PROVIDER_LABEL} selection index {idx} out of range")
}

pub(crate) fn query_search_credential_prompt_label() -> &'static str {
    "Query search credential env var name"
}

pub(crate) fn query_search_credential_source_validation_error(example_env_name: &str) -> String {
    format!(
        "{QUERY_SEARCH_CREDENTIAL_SOURCE_LABEL} must be an environment variable name like {example_env_name}"
    )
}

pub(crate) fn query_search_credential_clear_hint() -> &'static str {
    "clear the configured query search credential"
}

pub(crate) fn query_search_credential_input_hint(example_env_name: &str) -> String {
    format!(
        "enter the environment variable name only, for example {example_env_name}, or type :clear to remove the configured query search credential"
    )
}

pub(crate) fn set_query_search_credential_step(env_name: &str) -> String {
    format!("Set query search credential in env: {env_name}")
}

pub(crate) fn review_query_search_provider_choice_step(rerun_onboard_command: &str) -> String {
    format!(
        "Or rerun onboarding to review the query search provider choice: {rerun_onboard_command}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_search_terms_stay_canonical() {
        assert_eq!(QUERY_SEARCH_PROVIDER_LABEL, "query search provider");
        assert_eq!(QUERY_SEARCH_CREDENTIAL_LABEL, "query search credential");
        assert_eq!(
            QUERY_SEARCH_CREDENTIAL_SOURCE_LABEL,
            "query search credential source"
        );
        assert_eq!(CHOOSE_QUERY_SEARCH_TITLE, "choose query search");
        assert_eq!(
            CHOOSE_QUERY_SEARCH_PROVIDER_TITLE,
            "choose query search provider"
        );
        assert_eq!(
            CHOOSE_QUERY_SEARCH_CREDENTIAL_TITLE,
            "choose query search credential"
        );
        assert_eq!(ACCESS_BOUNDARY_LABEL, "access boundary");
    }
}
