#![cfg_attr(not(test), allow(dead_code))]

use tree_sitter::{Node, Parser};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BashCommandAnalysis {
    pub parse_unreliable: bool,
    pub units: Vec<MinimalCommandUnit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MinimalCommandUnit {
    pub classification: UnitClassification,
    pub preceding_operator: Option<UnitOperator>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UnitClassification {
    GovernablePlainCommand { argv: Vec<String> },
    Unsupported(UnsupportedStructureKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnsupportedStructureKind {
    BackgroundOperator,
    CommandSubstitution,
    CompoundCommand,
    EnvPrefixAssignment,
    Pipeline,
    ProcessSubstitution,
    Redirection,
    Subshell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnitOperator {
    Sequential,
    AndIf,
    OrIf,
}

pub(crate) fn analyze_bash_command(command: &str) -> BashCommandAnalysis {
    let mut parser = Parser::new();
    if parser
        .set_language(&tree_sitter_bash::LANGUAGE.into())
        .is_err()
    {
        return BashCommandAnalysis {
            parse_unreliable: true,
            units: Vec::new(),
        };
    }

    let Some(tree) = parser.parse(command, None) else {
        return BashCommandAnalysis {
            parse_unreliable: true,
            units: Vec::new(),
        };
    };

    let source = command.as_bytes();
    let root = tree.root_node();
    let mut units = Vec::new();
    let separators_supported = collect_program_units(root, source, &mut units);

    BashCommandAnalysis {
        parse_unreliable: root.has_error()
            || tree_contains_error_or_missing(root)
            || !separators_supported,
        units,
    }
}

fn collect_program_units(
    root: Node<'_>,
    source: &[u8],
    units: &mut Vec<MinimalCommandUnit>,
) -> bool {
    let mut reliable = true;
    let mut cursor = root.walk();
    let children: Vec<Node<'_>> = root.named_children(&mut cursor).collect();
    let mut previous_end = None;

    for child in children {
        let preceding_operator = if let Some(previous_end) = previous_end {
            let separator = source_text_between(source, previous_end, child.start_byte());
            if separator_supports_sequential_split(separator) {
                Some(UnitOperator::Sequential)
            } else {
                reliable = false;
                Some(UnitOperator::Sequential)
            }
        } else {
            None
        };

        reliable &= collect_statement_units(child, source, preceding_operator, units);
        previous_end = Some(child.end_byte());
    }

    reliable
}

fn collect_statement_units(
    node: Node<'_>,
    source: &[u8],
    preceding_operator: Option<UnitOperator>,
    units: &mut Vec<MinimalCommandUnit>,
) -> bool {
    if node.kind() == "list" {
        return collect_list_units(node, source, preceding_operator, units);
    }

    units.push(MinimalCommandUnit {
        classification: classify_statement(node, source),
        preceding_operator,
    });
    true
}

fn collect_list_units(
    list: Node<'_>,
    source: &[u8],
    preceding_operator: Option<UnitOperator>,
    units: &mut Vec<MinimalCommandUnit>,
) -> bool {
    let mut cursor = list.walk();
    let children: Vec<Node<'_>> = list.named_children(&mut cursor).collect();
    let [left, right] = children.as_slice() else {
        units.push(MinimalCommandUnit {
            classification: UnitClassification::Unsupported(
                UnsupportedStructureKind::CompoundCommand,
            ),
            preceding_operator,
        });
        return false;
    };
    let operator_text = source_text_between(source, left.end_byte(), right.start_byte());
    let Some(right_operator) = list_operator(operator_text) else {
        units.push(MinimalCommandUnit {
            classification: UnitClassification::Unsupported(
                UnsupportedStructureKind::CompoundCommand,
            ),
            preceding_operator,
        });
        return false;
    };

    let mut reliable = true;
    reliable &= collect_statement_units(*left, source, preceding_operator, units);
    reliable &= collect_statement_units(*right, source, Some(right_operator), units);
    reliable
}

fn classify_statement(node: Node<'_>, source: &[u8]) -> UnitClassification {
    match node.kind() {
        "command" => classify_command(node, source),
        "pipeline" => UnitClassification::Unsupported(UnsupportedStructureKind::Pipeline),
        "variable_assignment" | "variable_assignments" => {
            UnitClassification::Unsupported(UnsupportedStructureKind::EnvPrefixAssignment)
        }
        "redirected_statement" => {
            UnitClassification::Unsupported(UnsupportedStructureKind::Redirection)
        }
        "subshell" => UnitClassification::Unsupported(UnsupportedStructureKind::Subshell),
        "command_substitution" => {
            UnitClassification::Unsupported(UnsupportedStructureKind::CommandSubstitution)
        }
        "process_substitution" => {
            UnitClassification::Unsupported(UnsupportedStructureKind::ProcessSubstitution)
        }
        "function_definition"
        | "for_statement"
        | "c_style_for_statement"
        | "while_statement"
        | "if_statement"
        | "case_statement"
        | "compound_statement"
        | "test_command"
        | "negated_command" => {
            UnitClassification::Unsupported(UnsupportedStructureKind::CompoundCommand)
        }
        _ => UnitClassification::Unsupported(classify_descendant_unsupported_kind(node)),
    }
}

fn classify_command(node: Node<'_>, source: &[u8]) -> UnitClassification {
    if node.child_by_field_name("redirect").is_some() {
        return UnitClassification::Unsupported(UnsupportedStructureKind::Redirection);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "variable_assignment" | "variable_assignments" => {
                return UnitClassification::Unsupported(
                    UnsupportedStructureKind::EnvPrefixAssignment,
                );
            }
            "subshell" => {
                return UnitClassification::Unsupported(UnsupportedStructureKind::Subshell);
            }
            _ => {}
        }
    }

    if contains_kind(node, "command_substitution") {
        return UnitClassification::Unsupported(UnsupportedStructureKind::CommandSubstitution);
    }
    if contains_kind(node, "process_substitution") {
        return UnitClassification::Unsupported(UnsupportedStructureKind::ProcessSubstitution);
    }

    let Some(name) = node.child_by_field_name("name") else {
        return UnitClassification::Unsupported(UnsupportedStructureKind::CompoundCommand);
    };

    let mut argv = Vec::new();
    let Some(name_token) = extract_static_token(name, source) else {
        return UnitClassification::Unsupported(classify_descendant_unsupported_kind(name));
    };
    argv.push(name_token);

    let mut cursor = node.walk();
    for argument in node.children_by_field_name("argument", &mut cursor) {
        let Some(token) = extract_static_token(argument, source) else {
            return UnitClassification::Unsupported(classify_descendant_unsupported_kind(argument));
        };
        argv.push(token);
    }

    UnitClassification::GovernablePlainCommand { argv }
}

fn extract_static_token(node: Node<'_>, source: &[u8]) -> Option<String> {
    if contains_kind(node, "command_substitution")
        || contains_kind(node, "process_substitution")
        || contains_kind(node, "subshell")
        || contains_kind(node, "variable_assignment")
        || contains_kind(node, "variable_assignments")
    {
        return None;
    }

    match node.kind() {
        "command_name" | "word" | "raw_string" | "number" => {
            let text = node.utf8_text(source).ok()?.trim();
            (!text.is_empty()).then(|| text.to_owned())
        }
        "string" => {
            let text = node.utf8_text(source).ok()?.trim();
            let unquoted = text
                .strip_prefix('"')
                .and_then(|inner| inner.strip_suffix('"'))
                .or_else(|| {
                    text.strip_prefix('\'')
                        .and_then(|inner| inner.strip_suffix('\''))
                })
                .unwrap_or(text);
            (!unquoted.is_empty()).then(|| unquoted.to_owned())
        }
        _ => None,
    }
}

fn separator_supports_sequential_split(separator: &str) -> bool {
    let mut saw_supported_separator = false;

    for ch in separator.chars() {
        match ch {
            ';' | '\n' | '\r' => saw_supported_separator = true,
            ' ' | '\t' => {}
            _ => return false,
        }
    }

    saw_supported_separator
}

fn list_operator(separator: &str) -> Option<UnitOperator> {
    if separator.contains("&&") {
        Some(UnitOperator::AndIf)
    } else if separator.contains("||") {
        Some(UnitOperator::OrIf)
    } else {
        None
    }
}

fn classify_descendant_unsupported_kind(node: Node<'_>) -> UnsupportedStructureKind {
    if contains_kind(node, "command_substitution") {
        UnsupportedStructureKind::CommandSubstitution
    } else if contains_kind(node, "process_substitution") {
        UnsupportedStructureKind::ProcessSubstitution
    } else if contains_kind(node, "pipeline") {
        UnsupportedStructureKind::Pipeline
    } else if contains_kind(node, "variable_assignment")
        || contains_kind(node, "variable_assignments")
    {
        UnsupportedStructureKind::EnvPrefixAssignment
    } else if contains_kind(node, "subshell") {
        UnsupportedStructureKind::Subshell
    } else if contains_kind(node, "redirected_statement")
        || contains_kind(node, "file_redirect")
        || contains_kind(node, "herestring_redirect")
        || contains_kind(node, "heredoc_redirect")
    {
        UnsupportedStructureKind::Redirection
    } else if contains_kind(node, "list")
        || contains_kind(node, "function_definition")
        || contains_kind(node, "for_statement")
        || contains_kind(node, "c_style_for_statement")
        || contains_kind(node, "while_statement")
        || contains_kind(node, "if_statement")
        || contains_kind(node, "case_statement")
        || contains_kind(node, "compound_statement")
        || contains_kind(node, "test_command")
        || contains_kind(node, "negated_command")
    {
        UnsupportedStructureKind::CompoundCommand
    } else {
        UnsupportedStructureKind::BackgroundOperator
    }
}

fn contains_kind(node: Node<'_>, needle: &str) -> bool {
    if node.kind() == needle {
        return true;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if contains_kind(child, needle) {
            return true;
        }
    }

    false
}

fn tree_contains_error_or_missing(node: Node<'_>) -> bool {
    if node.is_error() || node.is_missing() {
        return true;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if tree_contains_error_or_missing(child) {
            return true;
        }
    }

    false
}

fn source_text_between(source: &[u8], start: usize, end: usize) -> &str {
    source
        .get(start..end)
        .and_then(|slice| std::str::from_utf8(slice).ok())
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_semicolon_lists_into_two_minimal_units() {
        let analysis = analyze_bash_command("cargo fmt ; cargo test");

        assert_eq!(analysis.units.len(), 2);
        assert_eq!(
            analysis.units[1].preceding_operator,
            Some(UnitOperator::Sequential)
        );
    }

    #[test]
    fn splits_and_and_or_lists_into_potentially_executable_units() {
        let analysis = analyze_bash_command("cd foo && cargo test || cargo test -- --nocapture");

        assert_eq!(analysis.units.len(), 3);
        assert_eq!(analysis.units[0].preceding_operator, None);
        assert_eq!(
            analysis.units[1].preceding_operator,
            Some(UnitOperator::AndIf)
        );
        assert_eq!(
            analysis.units[2].preceding_operator,
            Some(UnitOperator::OrIf)
        );
    }

    #[test]
    fn env_prefix_assignment_unit_is_classified_as_default_only() {
        let analysis = analyze_bash_command("FOO=1 cargo test");

        assert_eq!(
            analysis.units.first().map(|unit| &unit.classification),
            Some(&UnitClassification::Unsupported(
                UnsupportedStructureKind::EnvPrefixAssignment,
            ))
        );
    }

    #[test]
    fn pipeline_unit_is_classified_as_default_only() {
        let analysis = analyze_bash_command("cargo test | tee out.txt");

        assert_eq!(
            analysis.units.first().map(|unit| &unit.classification),
            Some(&UnitClassification::Unsupported(
                UnsupportedStructureKind::Pipeline,
            ))
        );
    }

    #[test]
    fn parse_error_marks_whole_command_unreliable() {
        let analysis = analyze_bash_command("if then");

        assert!(analysis.parse_unreliable);
    }

    #[test]
    fn command_substitution_unit_is_not_downgraded_to_plain_command() {
        let analysis = analyze_bash_command("echo $(git rev-parse HEAD)");

        assert_eq!(
            analysis.units.first().map(|unit| &unit.classification),
            Some(&UnitClassification::Unsupported(
                UnsupportedStructureKind::CommandSubstitution,
            ))
        );
    }
}
