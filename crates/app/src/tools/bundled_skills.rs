pub(crate) const BROWSER_COMPANION_PREVIEW_SKILL_ID: &str = "browser-companion-preview";
pub(crate) const BROWSER_COMPANION_PREVIEW_SOURCE_PATH: &str =
    "bundled://browser-companion-preview";
pub(crate) const BROWSER_COMPANION_COMMAND: &str = "agent-browser";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BundledExternalSkill {
    pub(crate) skill_id: &'static str,
    pub(crate) source_path: &'static str,
    pub(crate) instructions: &'static str,
}

const BROWSER_COMPANION_PREVIEW_INSTRUCTIONS: &str =
    include_str!("../../../../skills/browser-companion-preview/SKILL.md");

pub(crate) fn bundled_external_skill(skill_id: &str) -> Option<BundledExternalSkill> {
    match skill_id.trim() {
        BROWSER_COMPANION_PREVIEW_SKILL_ID => Some(BundledExternalSkill {
            skill_id: BROWSER_COMPANION_PREVIEW_SKILL_ID,
            source_path: BROWSER_COMPANION_PREVIEW_SOURCE_PATH,
            instructions: BROWSER_COMPANION_PREVIEW_INSTRUCTIONS,
        }),
        _ => None,
    }
}
