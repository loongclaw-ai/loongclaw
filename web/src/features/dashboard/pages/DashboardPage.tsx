import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Panel } from "../../../components/surfaces/Panel";
import { dashboardApi, type DashboardProviderItem, type DashboardSummary } from "../api";

type Tone = "good" | "warn" | "muted";

interface SummaryCard {
  key: string;
  value: string;
  chip: string;
  tone: Tone;
  items: Array<{ label: string; value: string }>;
}

export default function DashboardPage() {
  const { t } = useTranslation();
  const [summary, setSummary] = useState<DashboardSummary | null>(null);
  const [providers, setProviders] = useState<DashboardProviderItem[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function loadDashboard() {
      setError(null);
      try {
        const [loadedSummary, loadedProviders] = await Promise.all([
          dashboardApi.loadSummary(),
          dashboardApi.loadProviders(),
        ]);

        if (!cancelled) {
          setSummary(loadedSummary);
          setProviders(loadedProviders.items);
        }
      } catch (loadError) {
        if (!cancelled) {
          setError(loadError instanceof Error ? loadError.message : "Failed to load dashboard");
        }
      }
    }

    void loadDashboard();

    return () => {
      cancelled = true;
    };
  }, []);

  const activeProvider = providers.find((provider) => provider.enabled) ?? providers[0] ?? null;
  const runtimeTone: Tone =
    summary?.runtimeStatus === "ready" ? "good" : summary?.runtimeStatus ? "warn" : "muted";
  const providerTone: Tone = activeProvider?.enabled ? "good" : "muted";
  const apiKeyState = activeProvider?.apiKeyConfigured
    ? t("dashboard.values.configured")
    : t("dashboard.values.missing");

  const summaryCards: SummaryCard[] = [
    {
      key: "runtime",
      value: summary?.runtimeStatus ?? "Loading",
      chip: t("dashboard.values.live"),
      tone: runtimeTone,
      items: [
        { label: t("dashboard.fields.source"), value: t("dashboard.values.localDaemon") },
        { label: t("dashboard.fields.mode"), value: t("dashboard.values.readOnly") },
      ],
    },
    {
      key: "provider",
      value: summary?.activeProvider ?? t("dashboard.values.none"),
      chip: activeProvider?.enabled ? t("dashboard.values.active") : t("dashboard.values.inactive"),
      tone: providerTone,
      items: [
        { label: t("dashboard.fields.model"), value: summary?.activeModel ?? t("dashboard.values.noModel") },
        { label: t("dashboard.fields.profiles"), value: String(providers.length) },
      ],
    },
    {
      key: "memory",
      value: summary?.memoryBackend ?? t("dashboard.values.none"),
      chip: t("dashboard.values.stored"),
      tone: "muted",
      items: [
        { label: t("dashboard.fields.sessions"), value: String(summary?.sessionCount ?? 0) },
        { label: t("dashboard.fields.context"), value: t("dashboard.values.localStore") },
      ],
    },
    {
      key: "install",
      value: summary?.webInstallMode ?? t("dashboard.values.none"),
      chip: t("dashboard.values.optional"),
      tone: "muted",
      items: [
        { label: t("dashboard.fields.surface"), value: t("dashboard.values.webConsole") },
        { label: t("dashboard.fields.hosting"), value: t("dashboard.values.localOnly") },
      ],
    },
    {
      key: "tools",
      value: t("dashboard.values.readOnly"),
      chip: t("dashboard.values.phase1"),
      tone: "muted",
      items: [
        { label: t("dashboard.fields.shell"), value: t("dashboard.values.pending") },
        { label: t("dashboard.fields.web"), value: t("dashboard.values.connected") },
      ],
    },
  ];

  return (
    <div className="page">
      <section className="hero-block">
        <div className="hero-eyebrow">{t("dashboard.eyebrow")}</div>
        <h1 className="hero-title">{t("dashboard.title")}</h1>
        <p className="hero-subtitle">{t("dashboard.subtitle")}</p>
      </section>

      <section className="dashboard-summary-grid">
        {summaryCards.map((card) => (
          <article key={card.key} className="dashboard-stat-card">
            <div className="dashboard-stat-top">
              <div className="dashboard-stat-label">{t(`dashboard.cards.${card.key}`)}</div>
              <span className={`dashboard-pill dashboard-pill-${card.tone}`}>{card.chip}</span>
            </div>
            <div className="dashboard-stat-value">{card.value}</div>
            <div className="dashboard-stat-list">
              {card.items.map((item) => (
                <div key={item.label} className="dashboard-stat-row">
                  <span>{item.label}</span>
                  <strong title={item.value}>{item.value}</strong>
                </div>
              ))}
            </div>
          </article>
        ))}
      </section>

      {error ? <div className="empty-state dashboard-error">{error}</div> : null}

      <section className="dashboard-layout">
        <div className="dashboard-main-column">
          <Panel
            eyebrow={t("dashboard.sections.providerEyebrow")}
            title={t("dashboard.sections.providerTitle")}
            aside={
              <span className={`dashboard-pill dashboard-pill-${providerTone}`}>
                {activeProvider?.enabled ? t("dashboard.values.active") : t("dashboard.values.inactive")}
              </span>
            }
          >
            <div className="dashboard-provider-head">
              <div>
                <div className="dashboard-provider-name">
                  {activeProvider?.label ?? t("dashboard.values.none")}
                </div>
                <div
                  className="dashboard-provider-subtitle"
                  title={summary?.activeModel ?? t("dashboard.values.noModel")}
                >
                  {summary?.activeModel ?? t("dashboard.values.noModel")}
                </div>
              </div>

              <div className="dashboard-provider-meta">
                <div className="dashboard-meta-stack">
                  <span>{t("dashboard.fields.endpoint")}</span>
                  <strong title={activeProvider?.endpoint ?? t("dashboard.values.notSet")}>
                    {activeProvider?.endpoint ?? t("dashboard.values.notSet")}
                  </strong>
                </div>
                <div className="dashboard-meta-stack">
                  <span>{t("dashboard.fields.apiKey")}</span>
                  <strong>{apiKeyState}</strong>
                </div>
              </div>
            </div>

            <div className="dashboard-kv-grid">
              <div className="dashboard-kv-card">
                <span>{t("dashboard.fields.providerId")}</span>
                <strong>{activeProvider?.id ?? t("dashboard.values.none")}</strong>
              </div>
              <div className="dashboard-kv-card">
                <span>{t("dashboard.fields.defaultRole")}</span>
                <strong>
                  {activeProvider?.defaultForKind
                    ? t("dashboard.values.default")
                    : t("dashboard.values.secondary")}
                </strong>
              </div>
              <div className="dashboard-kv-card">
                <span>{t("dashboard.fields.profiles")}</span>
                <strong>{providers.length}</strong>
              </div>
              <div className="dashboard-kv-card">
                <span>{t("dashboard.fields.memory")}</span>
                <strong>{summary?.memoryBackend ?? t("dashboard.values.none")}</strong>
              </div>
            </div>

            <div className="dashboard-provider-list">
              {providers.length > 0 ? (
                providers.map((provider) => (
                  <div key={provider.id} className="dashboard-provider-item">
                    <div>
                      <div className="dashboard-provider-item-title">{provider.label}</div>
                      <div className="dashboard-provider-item-meta" title={provider.model}>
                        {provider.model}
                      </div>
                    </div>
                    <span className={`dashboard-pill dashboard-pill-${provider.enabled ? "good" : "muted"}`}>
                      {provider.enabled ? t("dashboard.values.active") : t("dashboard.values.standby")}
                    </span>
                  </div>
                ))
              ) : (
                <div className="empty-state">{t("dashboard.values.noProviders")}</div>
              )}
            </div>
          </Panel>

          <section className="dashboard-settings">
            <Panel
              eyebrow={t("dashboard.settings.eyebrow")}
              title={t("dashboard.settings.title")}
            >
              <div className="settings-header">
                <p className="panel-copy">{t("dashboard.settings.subtitle")}</p>
              </div>
              <form className="settings-form" onSubmit={(event) => event.preventDefault()}>
                <label className="settings-field">
                  <span className="settings-label">{t("dashboard.settings.activeProvider")}</span>
                  <select
                    className="settings-input"
                    defaultValue={activeProvider?.id ?? providers[0]?.id ?? ""}
                  >
                    {providers.map((provider) => (
                      <option key={provider.id} value={provider.id}>
                        {provider.label}
                      </option>
                    ))}
                  </select>
                </label>

                <label className="settings-field">
                  <span className="settings-label">{t("dashboard.settings.model")}</span>
                  <input className="settings-input" defaultValue={activeProvider?.model ?? ""} />
                </label>

                <label className="settings-field">
                  <span className="settings-label">{t("dashboard.settings.endpoint")}</span>
                  <input
                    className="settings-input"
                    defaultValue={activeProvider?.endpoint ?? ""}
                  />
                </label>

                <label className="settings-field">
                  <span className="settings-label">{t("dashboard.settings.apiKey")}</span>
                  <input
                    className="settings-input"
                    type="password"
                    defaultValue={activeProvider?.apiKeyMasked ?? ""}
                  />
                  <span className="settings-helper">{t("dashboard.settings.apiKeyMasked")}</span>
                </label>

                <div className="settings-actions">
                  <button type="button" className="hero-btn hero-btn-secondary">
                    {t("dashboard.settings.validate")}
                  </button>
                  <button type="submit" className="hero-btn hero-btn-primary">
                    {t("dashboard.settings.apply")}
                  </button>
                </div>

                <p className="settings-note">{t("dashboard.settings.helper")}</p>
              </form>
            </Panel>
          </section>
        </div>

        <div className="dashboard-side-column">
          <Panel
            eyebrow={t("dashboard.sections.runtimeEyebrow")}
            title={t("dashboard.sections.runtimeTitle")}
          >
            <div className="dashboard-kv-list">
              <div className="dashboard-kv-row">
                <span>{t("dashboard.fields.runtime")}</span>
                <strong title={summary?.runtimeStatus ?? "Loading"}>
                  {summary?.runtimeStatus ?? "Loading"}
                </strong>
              </div>
              <div className="dashboard-kv-row">
                <span>{t("dashboard.fields.provider")}</span>
                <strong title={summary?.activeProvider ?? t("dashboard.values.none")}>
                  {summary?.activeProvider ?? t("dashboard.values.none")}
                </strong>
              </div>
              <div className="dashboard-kv-row">
                <span>{t("dashboard.fields.model")}</span>
                <strong title={summary?.activeModel ?? t("dashboard.values.noModel")}>
                  {summary?.activeModel ?? t("dashboard.values.noModel")}
                </strong>
              </div>
              <div className="dashboard-kv-row">
                <span>{t("dashboard.fields.install")}</span>
                <strong title={summary?.webInstallMode ?? t("dashboard.values.none")}>
                  {summary?.webInstallMode ?? t("dashboard.values.none")}
                </strong>
              </div>
            </div>
          </Panel>

          <Panel
            eyebrow={t("dashboard.sections.diagnosticsEyebrow")}
            title={t("dashboard.sections.diagnosticsTitle")}
          >
            <div className="dashboard-kv-list">
              <div className="dashboard-kv-row">
                <span>{t("dashboard.fields.apiKey")}</span>
                <strong title={apiKeyState}>{apiKeyState}</strong>
              </div>
              <div className="dashboard-kv-row">
                <span>{t("dashboard.fields.sessions")}</span>
                <strong title={String(summary?.sessionCount ?? 0)}>{summary?.sessionCount ?? 0}</strong>
              </div>
              <div className="dashboard-kv-row">
                <span>{t("dashboard.fields.memory")}</span>
                <strong title={summary?.memoryBackend ?? t("dashboard.values.none")}>
                  {summary?.memoryBackend ?? t("dashboard.values.none")}
                </strong>
              </div>
              <div className="dashboard-kv-row">
                <span>{t("dashboard.fields.surface")}</span>
                <strong title={t("dashboard.values.webConsole")}>
                  {t("dashboard.values.webConsole")}
                </strong>
              </div>
            </div>
          </Panel>

          <Panel
            eyebrow={t("dashboard.sections.actionsEyebrow")}
            title={t("dashboard.sections.actionsTitle")}
          >
            <div className="dashboard-actions">
              <button type="button" className="hero-btn hero-btn-secondary">
                {t("dashboard.settings.validate")}
              </button>
              <button type="button" className="hero-btn hero-btn-secondary">
                {t("dashboard.actions.reload")}
              </button>
              <button type="button" className="hero-btn hero-btn-primary">
                {t("dashboard.settings.apply")}
              </button>
            </div>
            <p className="settings-note">{t("dashboard.actions.helper")}</p>
          </Panel>
        </div>
      </section>
    </div>
  );
}
