use crate::{
    RUNTIME_SNAPSHOT_ARTIFACT_JSON_SCHEMA_VERSION, RuntimeSnapshotArtifactDocument,
    sha2::{self, Digest},
};
use clap::{Args, Subcommand, ValueEnum};
use loongclaw_spec::CliResult;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

pub const RUNTIME_EXPERIMENT_ARTIFACT_JSON_SCHEMA_VERSION: u32 = 1;

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum RuntimeExperimentCommands {
    /// Create a new experiment-run artifact from a baseline runtime snapshot
    Start(RuntimeExperimentStartCommandOptions),
    /// Attach result snapshot and evaluation details to an experiment run
    Finish(RuntimeExperimentFinishCommandOptions),
    /// Load and render one persisted experiment-run artifact
    Show(RuntimeExperimentShowCommandOptions),
}

#[derive(Args, Debug, Clone, PartialEq, Eq)]
pub struct RuntimeExperimentStartCommandOptions {
    #[arg(long)]
    pub snapshot: String,
    #[arg(long)]
    pub output: String,
    #[arg(long)]
    pub mutation_summary: String,
    #[arg(long)]
    pub experiment_id: Option<String>,
    #[arg(long)]
    pub label: Option<String>,
    #[arg(long = "tag")]
    pub tag: Vec<String>,
    #[arg(long, default_value_t = false)]
    pub json: bool,
}

#[derive(Args, Debug, Clone, PartialEq, Eq)]
pub struct RuntimeExperimentFinishCommandOptions {
    #[arg(long)]
    pub run: String,
    #[arg(long)]
    pub result_snapshot: String,
    #[arg(long)]
    pub evaluation_summary: String,
    #[arg(long = "metric")]
    pub metric: Vec<String>,
    #[arg(long = "warning")]
    pub warning: Vec<String>,
    #[arg(long, value_enum)]
    pub decision: RuntimeExperimentDecision,
    #[arg(long, value_enum, default_value_t = RuntimeExperimentFinishStatus::Completed)]
    pub status: RuntimeExperimentFinishStatus,
    #[arg(long, default_value_t = false)]
    pub json: bool,
}

#[derive(Args, Debug, Clone, PartialEq, Eq)]
pub struct RuntimeExperimentShowCommandOptions {
    #[arg(long)]
    pub run: String,
    #[arg(long, default_value_t = false)]
    pub json: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeExperimentDecision {
    Undecided,
    Promoted,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeExperimentStatus {
    Planned,
    Completed,
    Aborted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RuntimeExperimentFinishStatus {
    Completed,
    Aborted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeExperimentArtifactSchema {
    pub version: u32,
    pub surface: String,
    pub purpose: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeExperimentMutationSummary {
    pub summary: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeExperimentSnapshotSummary {
    pub snapshot_id: String,
    pub created_at: String,
    pub label: Option<String>,
    pub experiment_id: Option<String>,
    pub parent_snapshot_id: Option<String>,
    pub capability_snapshot_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeExperimentEvaluation {
    pub summary: String,
    pub metrics: BTreeMap<String, f64>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeExperimentArtifactDocument {
    pub schema: RuntimeExperimentArtifactSchema,
    pub run_id: String,
    pub created_at: String,
    pub finished_at: Option<String>,
    pub label: Option<String>,
    pub experiment_id: String,
    pub status: RuntimeExperimentStatus,
    pub decision: RuntimeExperimentDecision,
    pub mutation: RuntimeExperimentMutationSummary,
    pub baseline_snapshot: RuntimeExperimentSnapshotSummary,
    pub result_snapshot: Option<RuntimeExperimentSnapshotSummary>,
    pub evaluation: Option<RuntimeExperimentEvaluation>,
}

pub fn run_runtime_experiment_cli(command: RuntimeExperimentCommands) -> CliResult<()> {
    match command {
        RuntimeExperimentCommands::Start(options) => {
            let as_json = options.json;
            let artifact = execute_runtime_experiment_start_command(options)?;
            emit_runtime_experiment_artifact(&artifact, as_json)
        }
        RuntimeExperimentCommands::Finish(options) => {
            let as_json = options.json;
            let artifact = execute_runtime_experiment_finish_command(options)?;
            emit_runtime_experiment_artifact(&artifact, as_json)
        }
        RuntimeExperimentCommands::Show(options) => {
            let as_json = options.json;
            let artifact = execute_runtime_experiment_show_command(options)?;
            emit_runtime_experiment_artifact(&artifact, as_json)
        }
    }
}

pub fn execute_runtime_experiment_start_command(
    options: RuntimeExperimentStartCommandOptions,
) -> CliResult<RuntimeExperimentArtifactDocument> {
    let baseline = load_runtime_snapshot_artifact(Path::new(&options.snapshot))?;
    let experiment_id = resolve_experiment_id(
        options.experiment_id.as_deref(),
        baseline.lineage.experiment_id.as_deref(),
    )?;
    let created_at = now_rfc3339()?;
    let label = optional_arg(options.label.as_deref());
    let mutation_summary = required_trimmed_arg("mutation_summary", &options.mutation_summary)?;
    let tags = normalize_repeated_values(&options.tag);
    let baseline_snapshot = build_snapshot_summary(&baseline);
    let run_id = compute_run_id(
        &created_at,
        label.as_deref(),
        &experiment_id,
        &baseline_snapshot,
        &mutation_summary,
        &tags,
    )?;
    let artifact = RuntimeExperimentArtifactDocument {
        schema: RuntimeExperimentArtifactSchema {
            version: RUNTIME_EXPERIMENT_ARTIFACT_JSON_SCHEMA_VERSION,
            surface: "runtime_experiment".to_owned(),
            purpose: "snapshot_evaluation_record".to_owned(),
        },
        run_id,
        created_at,
        finished_at: None,
        label,
        experiment_id,
        status: RuntimeExperimentStatus::Planned,
        decision: RuntimeExperimentDecision::Undecided,
        mutation: RuntimeExperimentMutationSummary {
            summary: mutation_summary,
            tags,
        },
        baseline_snapshot,
        result_snapshot: None,
        evaluation: None,
    };
    persist_runtime_experiment_artifact(&options.output, &artifact)?;
    Ok(artifact)
}

pub fn execute_runtime_experiment_finish_command(
    options: RuntimeExperimentFinishCommandOptions,
) -> CliResult<RuntimeExperimentArtifactDocument> {
    let mut artifact = load_runtime_experiment_artifact(Path::new(&options.run))?;
    if artifact.status != RuntimeExperimentStatus::Planned {
        return Err(format!(
            "runtime experiment run {} is already {}",
            options.run,
            render_status(artifact.status)
        ));
    }

    let result_snapshot = load_runtime_snapshot_artifact(Path::new(&options.result_snapshot))?;
    let result_experiment_id = optional_arg(result_snapshot.lineage.experiment_id.as_deref());
    if let Some(result_experiment_id) = result_experiment_id.as_deref()
        && result_experiment_id != artifact.experiment_id
    {
        return Err(format!(
            "runtime experiment result snapshot experiment_id `{result_experiment_id}` does not match run experiment_id `{}`",
            artifact.experiment_id
        ));
    }

    let mut warnings = normalize_warnings(&options.warning);
    if result_experiment_id.is_none() {
        warnings.push(format!(
            "result snapshot {} is missing experiment_id; operator-confirmed lineage is required",
            options.result_snapshot
        ));
    }

    artifact.finished_at = Some(now_rfc3339()?);
    artifact.status = match options.status {
        RuntimeExperimentFinishStatus::Completed => RuntimeExperimentStatus::Completed,
        RuntimeExperimentFinishStatus::Aborted => RuntimeExperimentStatus::Aborted,
    };
    artifact.decision = options.decision;
    artifact.result_snapshot = Some(build_snapshot_summary(&result_snapshot));
    artifact.evaluation = Some(RuntimeExperimentEvaluation {
        summary: required_trimmed_arg("evaluation_summary", &options.evaluation_summary)?,
        metrics: parse_metrics(&options.metric)?,
        warnings,
    });
    persist_runtime_experiment_artifact(&options.run, &artifact)?;
    Ok(artifact)
}

pub fn execute_runtime_experiment_show_command(
    options: RuntimeExperimentShowCommandOptions,
) -> CliResult<RuntimeExperimentArtifactDocument> {
    load_runtime_experiment_artifact(Path::new(&options.run))
}

fn emit_runtime_experiment_artifact(
    artifact: &RuntimeExperimentArtifactDocument,
    as_json: bool,
) -> CliResult<()> {
    if as_json {
        let pretty = serde_json::to_string_pretty(artifact)
            .map_err(|error| format!("serialize runtime experiment artifact failed: {error}"))?;
        println!("{pretty}");
        return Ok(());
    }

    println!("{}", render_runtime_experiment_text(artifact));
    Ok(())
}

fn load_runtime_snapshot_artifact(path: &Path) -> CliResult<RuntimeSnapshotArtifactDocument> {
    let raw = fs::read_to_string(path).map_err(|error| {
        format!(
            "read runtime snapshot artifact {} failed: {error}",
            path.display()
        )
    })?;
    let artifact =
        serde_json::from_str::<RuntimeSnapshotArtifactDocument>(&raw).map_err(|error| {
            format!(
                "decode runtime snapshot artifact {} failed: {error}",
                path.display()
            )
        })?;
    if artifact.schema.version != RUNTIME_SNAPSHOT_ARTIFACT_JSON_SCHEMA_VERSION {
        return Err(format!(
            "runtime snapshot artifact {} uses unsupported schema version {}; expected {}",
            path.display(),
            artifact.schema.version,
            RUNTIME_SNAPSHOT_ARTIFACT_JSON_SCHEMA_VERSION
        ));
    }
    Ok(artifact)
}

fn load_runtime_experiment_artifact(path: &Path) -> CliResult<RuntimeExperimentArtifactDocument> {
    let raw = fs::read_to_string(path).map_err(|error| {
        format!(
            "read runtime experiment artifact {} failed: {error}",
            path.display()
        )
    })?;
    let artifact =
        serde_json::from_str::<RuntimeExperimentArtifactDocument>(&raw).map_err(|error| {
            format!(
                "decode runtime experiment artifact {} failed: {error}",
                path.display()
            )
        })?;
    if artifact.schema.version != RUNTIME_EXPERIMENT_ARTIFACT_JSON_SCHEMA_VERSION {
        return Err(format!(
            "runtime experiment artifact {} uses unsupported schema version {}; expected {}",
            path.display(),
            artifact.schema.version,
            RUNTIME_EXPERIMENT_ARTIFACT_JSON_SCHEMA_VERSION
        ));
    }
    Ok(artifact)
}

fn resolve_experiment_id(explicit: Option<&str>, baseline: Option<&str>) -> CliResult<String> {
    let explicit = optional_arg(explicit);
    let baseline = optional_arg(baseline);
    match (explicit, baseline) {
        (Some(explicit), Some(baseline)) if explicit != baseline => Err(format!(
            "runtime experiment start --experiment-id `{explicit}` does not match baseline snapshot experiment_id `{baseline}`"
        )),
        (Some(explicit), _) => Ok(explicit),
        (None, Some(baseline)) => Ok(baseline),
        (None, None) => Err(
            "runtime experiment start requires --experiment-id when the baseline snapshot artifact does not declare experiment_id".to_owned(),
        ),
    }
}

fn now_rfc3339() -> CliResult<String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| format!("format runtime experiment timestamp failed: {error}"))
}

fn optional_arg(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn required_trimmed_arg(name: &str, raw: &str) -> CliResult<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(format!("runtime experiment {name} cannot be empty"));
    }
    Ok(trimmed.to_owned())
}

fn normalize_repeated_values(values: &[String]) -> Vec<String> {
    let mut normalized = values
        .iter()
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    normalized.sort();
    normalized
}

fn normalize_warnings(values: &[String]) -> Vec<String> {
    normalize_repeated_values(values)
}

fn parse_metrics(values: &[String]) -> CliResult<BTreeMap<String, f64>> {
    let mut metrics = BTreeMap::new();
    for raw in values {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err("runtime experiment metric entries cannot be empty".to_owned());
        }
        let (key, value) = trimmed.split_once('=').ok_or_else(|| {
            format!("runtime experiment metric `{trimmed}` must use key=value syntax")
        })?;
        let key = key.trim();
        if key.is_empty() {
            return Err(format!(
                "runtime experiment metric `{trimmed}` is missing a metric name"
            ));
        }
        let value = value.trim().parse::<f64>().map_err(|error| {
            format!("runtime experiment metric `{trimmed}` must be numeric: {error}")
        })?;
        if metrics.insert(key.to_owned(), value).is_some() {
            return Err(format!(
                "runtime experiment metric `{key}` was provided more than once"
            ));
        }
    }
    Ok(metrics)
}

fn build_snapshot_summary(
    artifact: &RuntimeSnapshotArtifactDocument,
) -> RuntimeExperimentSnapshotSummary {
    RuntimeExperimentSnapshotSummary {
        snapshot_id: artifact.lineage.snapshot_id.clone(),
        created_at: artifact.lineage.created_at.clone(),
        label: artifact.lineage.label.clone(),
        experiment_id: artifact.lineage.experiment_id.clone(),
        parent_snapshot_id: artifact.lineage.parent_snapshot_id.clone(),
        capability_snapshot_sha256: artifact
            .tools
            .get("capability_snapshot_sha256")
            .and_then(Value::as_str)
            .map(str::to_owned),
    }
}

fn compute_run_id(
    created_at: &str,
    label: Option<&str>,
    experiment_id: &str,
    baseline_snapshot: &RuntimeExperimentSnapshotSummary,
    mutation_summary: &str,
    tags: &[String],
) -> CliResult<String> {
    let encoded = serde_json::to_vec(&json!({
        "created_at": created_at,
        "label": label,
        "experiment_id": experiment_id,
        "baseline_snapshot_id": baseline_snapshot.snapshot_id,
        "mutation_summary": mutation_summary,
        "tags": tags,
    }))
    .map_err(|error| format!("serialize runtime experiment run_id input failed: {error}"))?;
    Ok(format!("{:x}", sha2::Sha256::digest(encoded)))
}

fn persist_runtime_experiment_artifact(
    output: &str,
    artifact: &RuntimeExperimentArtifactDocument,
) -> CliResult<()> {
    let output_path = PathBuf::from(output);
    if let Some(parent) = output_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "create runtime experiment artifact directory {} failed: {error}",
                parent.display()
            )
        })?;
    }
    let encoded = serde_json::to_string_pretty(artifact)
        .map_err(|error| format!("serialize runtime experiment artifact failed: {error}"))?;
    fs::write(&output_path, encoded).map_err(|error| {
        format!(
            "write runtime experiment artifact {} failed: {error}",
            output_path.display()
        )
    })?;
    Ok(())
}

pub fn render_runtime_experiment_text(artifact: &RuntimeExperimentArtifactDocument) -> String {
    let metrics = artifact
        .evaluation
        .as_ref()
        .map(|evaluation| {
            if evaluation.metrics.is_empty() {
                "-".to_owned()
            } else {
                evaluation
                    .metrics
                    .iter()
                    .map(|(key, value)| format!("{key}:{value}"))
                    .collect::<Vec<_>>()
                    .join(",")
            }
        })
        .unwrap_or_else(|| "-".to_owned());
    let warnings = artifact
        .evaluation
        .as_ref()
        .map(|evaluation| {
            if evaluation.warnings.is_empty() {
                "-".to_owned()
            } else {
                evaluation.warnings.join(" | ")
            }
        })
        .unwrap_or_else(|| "-".to_owned());

    [
        format!("run_id={}", artifact.run_id),
        format!("experiment_id={}", artifact.experiment_id),
        format!(
            "baseline_snapshot_id={}",
            artifact.baseline_snapshot.snapshot_id
        ),
        format!(
            "result_snapshot_id={}",
            artifact
                .result_snapshot
                .as_ref()
                .map(|snapshot| snapshot.snapshot_id.as_str())
                .unwrap_or("-")
        ),
        format!("status={}", render_status(artifact.status)),
        format!("decision={}", render_decision(artifact.decision)),
        format!("metrics={metrics}"),
        format!("warnings={warnings}"),
        format!("mutation_summary={}", artifact.mutation.summary),
        format!(
            "mutation_tags={}",
            if artifact.mutation.tags.is_empty() {
                "-".to_owned()
            } else {
                artifact.mutation.tags.join(",")
            }
        ),
    ]
    .join("\n")
}

fn render_status(status: RuntimeExperimentStatus) -> &'static str {
    match status {
        RuntimeExperimentStatus::Planned => "planned",
        RuntimeExperimentStatus::Completed => "completed",
        RuntimeExperimentStatus::Aborted => "aborted",
    }
}

fn render_decision(decision: RuntimeExperimentDecision) -> &'static str {
    match decision {
        RuntimeExperimentDecision::Undecided => "undecided",
        RuntimeExperimentDecision::Promoted => "promoted",
        RuntimeExperimentDecision::Rejected => "rejected",
    }
}
