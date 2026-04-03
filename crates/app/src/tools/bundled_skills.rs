use include_dir::{Dir, include_dir};

pub(crate) const BROWSER_COMPANION_PREVIEW_SKILL_ID: &str = "browser-companion-preview";
pub(crate) const BROWSER_COMPANION_COMMAND: &str = "agent-browser";

static BUNDLED_SKILLS_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../skills");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BundledExternalSkill {
    pub(crate) skill_id: &'static str,
    pub(crate) source_path: &'static str,
    pub(crate) relative_dir: &'static str,
}

const BUNDLED_EXTERNAL_SKILLS: &[BundledExternalSkill] = &[
    BundledExternalSkill {
        skill_id: "agent-browser",
        source_path: "bundled://agent-browser",
        relative_dir: "agent-browser",
    },
    BundledExternalSkill {
        skill_id: BROWSER_COMPANION_PREVIEW_SKILL_ID,
        source_path: "bundled://browser-companion-preview",
        relative_dir: "browser-companion-preview",
    },
    BundledExternalSkill {
        skill_id: "design-md",
        source_path: "bundled://design-md",
        relative_dir: "design-md",
    },
    BundledExternalSkill {
        skill_id: "docx",
        source_path: "bundled://docx",
        relative_dir: "docx",
    },
    BundledExternalSkill {
        skill_id: "find-skills",
        source_path: "bundled://find-skills",
        relative_dir: "find-skills",
    },
    BundledExternalSkill {
        skill_id: "github-issues",
        source_path: "bundled://github-issues",
        relative_dir: "github-issues",
    },
    BundledExternalSkill {
        skill_id: "lark-approval",
        source_path: "bundled://lark-approval",
        relative_dir: "lark-approval",
    },
    BundledExternalSkill {
        skill_id: "lark-base",
        source_path: "bundled://lark-base",
        relative_dir: "lark-base",
    },
    BundledExternalSkill {
        skill_id: "lark-calendar",
        source_path: "bundled://lark-calendar",
        relative_dir: "lark-calendar",
    },
    BundledExternalSkill {
        skill_id: "lark-contact",
        source_path: "bundled://lark-contact",
        relative_dir: "lark-contact",
    },
    BundledExternalSkill {
        skill_id: "lark-doc",
        source_path: "bundled://lark-doc",
        relative_dir: "lark-doc",
    },
    BundledExternalSkill {
        skill_id: "lark-drive",
        source_path: "bundled://lark-drive",
        relative_dir: "lark-drive",
    },
    BundledExternalSkill {
        skill_id: "lark-event",
        source_path: "bundled://lark-event",
        relative_dir: "lark-event",
    },
    BundledExternalSkill {
        skill_id: "lark-im",
        source_path: "bundled://lark-im",
        relative_dir: "lark-im",
    },
    BundledExternalSkill {
        skill_id: "lark-mail",
        source_path: "bundled://lark-mail",
        relative_dir: "lark-mail",
    },
    BundledExternalSkill {
        skill_id: "lark-minutes",
        source_path: "bundled://lark-minutes",
        relative_dir: "lark-minutes",
    },
    BundledExternalSkill {
        skill_id: "lark-openapi-explorer",
        source_path: "bundled://lark-openapi-explorer",
        relative_dir: "lark-openapi-explorer",
    },
    BundledExternalSkill {
        skill_id: "lark-shared",
        source_path: "bundled://lark-shared",
        relative_dir: "lark-shared",
    },
    BundledExternalSkill {
        skill_id: "lark-sheets",
        source_path: "bundled://lark-sheets",
        relative_dir: "lark-sheets",
    },
    BundledExternalSkill {
        skill_id: "lark-skill-maker",
        source_path: "bundled://lark-skill-maker",
        relative_dir: "lark-skill-maker",
    },
    BundledExternalSkill {
        skill_id: "lark-task",
        source_path: "bundled://lark-task",
        relative_dir: "lark-task",
    },
    BundledExternalSkill {
        skill_id: "lark-vc",
        source_path: "bundled://lark-vc",
        relative_dir: "lark-vc",
    },
    BundledExternalSkill {
        skill_id: "lark-whiteboard",
        source_path: "bundled://lark-whiteboard",
        relative_dir: "lark-whiteboard",
    },
    BundledExternalSkill {
        skill_id: "lark-wiki",
        source_path: "bundled://lark-wiki",
        relative_dir: "lark-wiki",
    },
    BundledExternalSkill {
        skill_id: "lark-workflow-meeting-summary",
        source_path: "bundled://lark-workflow-meeting-summary",
        relative_dir: "lark-workflow-meeting-summary",
    },
    BundledExternalSkill {
        skill_id: "lark-workflow-standup-report",
        source_path: "bundled://lark-workflow-standup-report",
        relative_dir: "lark-workflow-standup-report",
    },
    BundledExternalSkill {
        skill_id: "pdf",
        source_path: "bundled://pdf",
        relative_dir: "pdf",
    },
    BundledExternalSkill {
        skill_id: "plan",
        source_path: "bundled://plan",
        relative_dir: "plan",
    },
    BundledExternalSkill {
        skill_id: "pptx",
        source_path: "bundled://pptx",
        relative_dir: "pptx",
    },
    BundledExternalSkill {
        skill_id: "skill-creator",
        source_path: "bundled://skill-creator",
        relative_dir: "skill-creator",
    },
    BundledExternalSkill {
        skill_id: "systematic-debugging",
        source_path: "bundled://systematic-debugging",
        relative_dir: "systematic-debugging",
    },
    BundledExternalSkill {
        skill_id: "xlsx",
        source_path: "bundled://xlsx",
        relative_dir: "xlsx",
    },
    BundledExternalSkill {
        skill_id: "mcporter",
        source_path: "bundled://mcporter",
        relative_dir: "mcporter",
    },
    BundledExternalSkill {
        skill_id: "minimax-docx",
        source_path: "bundled://minimax-docx",
        relative_dir: "minimax-docx",
    },
    BundledExternalSkill {
        skill_id: "minimax-pdf",
        source_path: "bundled://minimax-pdf",
        relative_dir: "minimax-pdf",
    },
    BundledExternalSkill {
        skill_id: "minimax-xlsx",
        source_path: "bundled://minimax-xlsx",
        relative_dir: "minimax-xlsx",
    },
    BundledExternalSkill {
        skill_id: "native-mcp",
        source_path: "bundled://native-mcp",
        relative_dir: "native-mcp",
    },
];

pub(crate) fn bundled_external_skills() -> &'static [BundledExternalSkill] {
    BUNDLED_EXTERNAL_SKILLS
}

pub(crate) fn bundled_external_skill(skill_id: &str) -> Option<BundledExternalSkill> {
    bundled_external_skills()
        .iter()
        .copied()
        .find(|skill| skill.skill_id == skill_id.trim())
}

pub(crate) fn bundled_external_skill_dir(
    skill: &BundledExternalSkill,
) -> Option<&'static Dir<'static>> {
    BUNDLED_SKILLS_DIR.get_dir(skill.relative_dir)
}

pub(crate) fn bundled_external_skill_markdown(
    skill: &BundledExternalSkill,
) -> Result<&'static str, String> {
    let dir = bundled_external_skill_dir(skill)
        .ok_or_else(|| format!("missing bundled skill directory `{}`", skill.relative_dir))?;
    let file = dir
        .entries()
        .iter()
        .find_map(|entry| match entry {
            include_dir::DirEntry::File(file)
                if file
                    .path()
                    .file_name()
                    .is_some_and(|name| name == "SKILL.md") =>
            {
                Some(file)
            }
            include_dir::DirEntry::Dir(_) | include_dir::DirEntry::File(_) => None,
        })
        .ok_or_else(|| format!("missing bundled SKILL.md for `{}`", skill.skill_id))?;
    std::str::from_utf8(file.contents()).map_err(|error| {
        format!(
            "bundled SKILL.md for `{}` is not utf-8: {error}",
            skill.skill_id
        )
    })
}

#[cfg(test)]
mod tests {
    use super::bundled_external_skill;

    #[test]
    fn curated_bundled_inventory_contains_requested_preinstalls() {
        for skill_id in [
            "find-skills",
            "agent-browser",
            "skill-creator",
            "pdf",
            "docx",
            "pptx",
            "xlsx",
            "minimax-docx",
            "minimax-pdf",
            "minimax-xlsx",
            "design-md",
            "lark-doc",
            "native-mcp",
            "mcporter",
            "github-issues",
            "systematic-debugging",
            "plan",
        ] {
            assert!(
                bundled_external_skill(skill_id).is_some(),
                "expected bundled skill inventory to expose `{skill_id}`"
            );
        }
    }
}
