import { Plus, SendHorizontal, Trash2 } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { ReactNode } from "react";
import { Panel } from "../../../components/surfaces/Panel";
import { useWebConnection } from "../../../hooks/useWebConnection";
import { ApiRequestError } from "../../../lib/api/client";
import { dashboardApi } from "../../dashboard/api";
import { useChatSessions } from "../hooks/useChatSessions";
import { useChatStream } from "../hooks/useChatStream";

function renderInlineBreaks(text: string): ReactNode[] {
  return text.split("\n").flatMap((line, index, lines) => {
    const nodes: ReactNode[] = [line];
    if (index < lines.length - 1) {
      nodes.push(<br key={`br-${index}`} />);
    }
    return nodes;
  });
}

function renderInlineContent(text: string): ReactNode[] {
  const parts = text.split(/(`[^`]+`)/g);
  return parts.flatMap((part, index) => {
    if (!part) {
      return [];
    }

    if (part.startsWith("`") && part.endsWith("`") && part.length >= 2) {
      return [<code key={`inline-code-${index}`}>{part.slice(1, -1)}</code>];
    }

    return renderInlineBreaks(part).map((node, nodeIndex) => (
      <span key={`inline-text-${index}-${nodeIndex}`}>{node}</span>
    ));
  });
}

type MessageBlock =
  | { type: "heading"; level: 1 | 2 | 3; text: string }
  | { type: "paragraph"; text: string }
  | { type: "unordered-list"; items: string[] }
  | { type: "ordered-list"; items: string[] }
  | { type: "blockquote"; text: string }
  | { type: "code"; language: string | null; text: string }
  | { type: "table"; headers: string[]; rows: string[][] };

function isTableDivider(line: string): boolean {
  const normalized = line.trim();
  if (!normalized.includes("|")) {
    return false;
  }

  const cells = normalized
    .split("|")
    .map((cell) => cell.trim())
    .filter(Boolean);

  return (
    cells.length > 0 &&
    cells.every((cell) => /^:?-{3,}:?$/.test(cell))
  );
}

function parseTableRow(line: string): string[] {
  return line
    .trim()
    .replace(/^\|/, "")
    .replace(/\|$/, "")
    .split("|")
    .map((cell) => cell.trim());
}

function renderMessageContent(content: string): ReactNode[] {
  const normalized = content.replace(/\r\n/g, "\n");
  const rawLines = normalized.split("\n");
  let start = 0;
  let end = rawLines.length;

  while (start < end && rawLines[start].trim() === "") {
    start += 1;
  }
  while (end > start && rawLines[end - 1].trim() === "") {
    end -= 1;
  }

  const lines = rawLines.slice(start, end);
  if (lines.length === 0) {
    return [];
  }

  const parsedBlocks: MessageBlock[] = [];
  let paragraphLines: string[] = [];
  let listItems: string[] = [];
  let listType: "unordered-list" | "ordered-list" | null = null;
  let quoteLines: string[] = [];

  function flushParagraph() {
    if (paragraphLines.length === 0) {
      return;
    }
    const text = paragraphLines.join("\n").trim();
    if (text) {
      parsedBlocks.push({ type: "paragraph", text });
    }
    paragraphLines = [];
  }

  function flushList() {
    if (listItems.length === 0) {
      return;
    }
    parsedBlocks.push({
      type: listType ?? "unordered-list",
      items: [...listItems],
    });
    listItems = [];
    listType = null;
  }

  function flushQuote() {
    if (quoteLines.length === 0) {
      return;
    }
    const text = quoteLines.join("\n").trim();
    if (text) {
      parsedBlocks.push({ type: "blockquote", text });
    }
    quoteLines = [];
  }

  for (let index = 0; index < lines.length; index += 1) {
    const rawLine = lines[index];
    const trimmedLine = rawLine.trim();

    if (!trimmedLine) {
      flushParagraph();
      flushList();
      flushQuote();
      continue;
    }

    const fenceMatch = rawLine.match(/^\s*```([^`]*)$/);
    if (fenceMatch) {
      flushParagraph();
      flushList();
      flushQuote();
      const codeLines: string[] = [];
      const language = fenceMatch[1].trim() || null;
      index += 1;
      while (index < lines.length && !lines[index].match(/^\s*```/)) {
        codeLines.push(lines[index]);
        index += 1;
      }
      parsedBlocks.push({
        type: "code",
        language,
        text: codeLines.join("\n"),
      });
      continue;
    }

    if (
      index + 1 < lines.length &&
      trimmedLine.includes("|") &&
      isTableDivider(lines[index + 1])
    ) {
      flushParagraph();
      flushList();
      flushQuote();
      const headers = parseTableRow(trimmedLine);
      const rows: string[][] = [];
      index += 2;
      while (index < lines.length && lines[index].trim().includes("|")) {
        rows.push(parseTableRow(lines[index]));
        index += 1;
      }
      index -= 1;
      parsedBlocks.push({ type: "table", headers, rows });
      continue;
    }

    const headingMatch = trimmedLine.match(/^(#{1,3})\s+(.+)$/);
    if (headingMatch) {
      flushParagraph();
      flushList();
      flushQuote();
      const level = headingMatch[1].length;
      const title = headingMatch[2];
      parsedBlocks.push({ type: "heading", level: level as 1 | 2 | 3, text: title });
      continue;
    }

    const unorderedListMatch = trimmedLine.match(/^[-*]\s+(.+)$/);
    if (unorderedListMatch) {
      flushParagraph();
      flushQuote();
      if (listType && listType !== "unordered-list") {
        flushList();
      }
      listType = "unordered-list";
      listItems.push(unorderedListMatch[1]);
      continue;
    }

    const orderedListMatch = trimmedLine.match(/^\d+\.\s+(.+)$/);
    if (orderedListMatch) {
      flushParagraph();
      flushQuote();
      if (listType && listType !== "ordered-list") {
        flushList();
      }
      listType = "ordered-list";
      listItems.push(orderedListMatch[1]);
      continue;
    }

    const quoteMatch = rawLine.match(/^\s*>\s?(.*)$/);
    if (quoteMatch) {
      flushParagraph();
      flushList();
      quoteLines.push(quoteMatch[1]);
      continue;
    }

    flushList();
    flushQuote();
    paragraphLines.push(rawLine.trimEnd());
  }

  flushParagraph();
  flushList();
  flushQuote();

  return parsedBlocks.map((block, blockIndex) => {
    switch (block.type) {
      case "heading":
        if (block.level === 1) {
          return <h1 key={`block-${blockIndex}`}>{block.text}</h1>;
        }
        if (block.level === 2) {
          return <h2 key={`block-${blockIndex}`}>{block.text}</h2>;
        }
        return <h3 key={`block-${blockIndex}`}>{block.text}</h3>;
      case "paragraph":
        return <p key={`block-${blockIndex}`}>{renderInlineContent(block.text)}</p>;
      case "unordered-list":
        return (
          <ul key={`block-${blockIndex}`}>
            {block.items.map((item, itemIndex) => (
              <li key={`item-${itemIndex}`}>{renderInlineContent(item)}</li>
            ))}
          </ul>
        );
      case "ordered-list":
        return (
          <ol key={`block-${blockIndex}`}>
            {block.items.map((item, itemIndex) => (
              <li key={`item-${itemIndex}`}>{renderInlineContent(item)}</li>
            ))}
          </ol>
        );
      case "blockquote":
        return (
          <blockquote key={`block-${blockIndex}`}>
            {renderInlineContent(block.text)}
          </blockquote>
        );
      case "code":
        return (
          <pre key={`block-${blockIndex}`}>
            {block.language ? <span className="message-code-language">{block.language}</span> : null}
            <code>{block.text}</code>
          </pre>
        );
      case "table":
        return (
          <div key={`block-${blockIndex}`} className="message-table-wrap">
            <table className="message-table">
              <thead>
                <tr>
                  {block.headers.map((header, headerIndex) => (
                    <th key={`header-${headerIndex}`}>{renderInlineContent(header)}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {block.rows.map((row, rowIndex) => (
                  <tr key={`row-${rowIndex}`}>
                    {row.map((cell, cellIndex) => (
                      <td key={`cell-${rowIndex}-${cellIndex}`}>{renderInlineContent(cell)}</td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        );
      default:
        return null;
    }
  });
}



export default function ChatPage() {
  const { t } = useTranslation();
  const connection = useWebConnection();
  const { canAccessProtectedApi, authRevision, markUnauthorized } = connection;

  const [composerText, setComposerText] = useState("");
  const [memoryWindow, setMemoryWindow] = useState<number | null>(null);
  const [currentModel, setCurrentModel] = useState("");
  const [currentProvider, setCurrentProvider] = useState<string | null>(null);
  const [loadingLabelIndex, setLoadingLabelIndex] = useState(0);
  const messageListRef = useRef<HTMLDivElement | null>(null);
  const shouldAutoScrollRef = useRef(true);

  const sessionsState = useChatSessions(t);
  const {
    sessions,
    selectedSessionId,
    messages,
    activeTools,
    pendingAssistantId,
    streamPhase,
    isLoadingSessions,
    isLoadingHistory,
    deletingSessionId,
    error,
    setError,
    selectSession,
    upsertSession,
    refreshSessions,
    deleteSession,
    updateSessionViewState,
  } = sessionsState;

  const streamState = useChatStream({
    t,
    sessionId: selectedSessionId,
    canAccessProtectedApi,
    markUnauthorized,
    authMode: connection.authMode,
    tokenPath: connection.tokenPath,
    tokenEnv: connection.tokenEnv,
    updateSessionViewState,
    selectSession,
    upsertSession,
    removeSession: sessionsState.removeSession,
    refreshSessions,
    setError,
  });

  const { isSubmitting, sendMessage, stopStream } = streamState;

  const loadingPhraseKeys = useMemo(() => {
    switch (streamPhase) {
      case "connecting":
        return [
          "chat.loading.connectingA",
          "chat.loading.connectingB",
          "chat.loading.connectingC",
        ];
      case "thinking":
        return [
          "chat.loading.thinkingA",
          "chat.loading.thinkingB",
          "chat.loading.thinkingC",
        ];
      case "streaming":
        return [
          "chat.loading.streamingA",
          "chat.loading.streamingB",
          "chat.loading.streamingC",
        ];
      default:
        return [];
    }
  }, [streamPhase]);

  const loadingPhrases = useMemo(
    () => loadingPhraseKeys.map((key) => t(key)),
    [loadingPhraseKeys, t],
  );
  const loadingLabelBase =
    loadingPhrases.length > 0
      ? loadingPhrases[loadingLabelIndex % loadingPhrases.length]
      : t("chat.generating");
  useEffect(() => {
    if (streamPhase === "idle" || loadingPhrases.length <= 1) {
      setLoadingLabelIndex(0);
      return;
    }

    const intervalId = window.setInterval(() => {
      setLoadingLabelIndex((current) => (current + 1) % loadingPhrases.length);
    }, 2200);

    return () => {
      window.clearInterval(intervalId);
    };
  }, [loadingPhrases.length, streamPhase]);

  useEffect(() => {
    let cancelled = false;
    async function loadConfigSnapshot() {
      try {
        const config = await dashboardApi.loadConfig();
        if (!cancelled) {
          setMemoryWindow(config.slidingWindow);
          setCurrentModel(config.model);
          setCurrentProvider(config.activeProvider);
        }
      } catch (loadError) {
        if (!cancelled && loadError instanceof ApiRequestError && loadError.status === 401) {
          markUnauthorized();
        }
      }
    }
    if (canAccessProtectedApi) void loadConfigSnapshot();
    return () => { cancelled = true; };
  }, [authRevision, canAccessProtectedApi, markUnauthorized]);

  useEffect(() => {
    shouldAutoScrollRef.current = true;
  }, [selectedSessionId]);

  useEffect(() => {
    if (!messageListRef.current || !shouldAutoScrollRef.current) return;
    const container = messageListRef.current;
    container.scrollTop = container.scrollHeight;
  }, [messages, isLoadingHistory, selectedSessionId]);

  async function handleSubmit() {
    const input = composerText.trim();
    if (!input) return;
    setComposerText("");
    const accepted = await sendMessage(input);
    if (accepted === false) {
      setComposerText(input);
    }
  }

  const selectedSession =
    sessions.find((session) => session.id === selectedSessionId) ?? null;

  return (
    <div className="page page-chat">
      <div className="chat-shell">
        <Panel
          eyebrow={t("chat.panels.sessions")}
          title={t("chat.newSession")}
          aside={
            <button
              type="button"
              className="panel-action"
              onClick={() => {
                selectSession(null);
                setError(null);
              }}
              disabled={!canAccessProtectedApi}
            >
              <Plus size={14} />
            </button>
          }
        >
          <div className="stack-list stack-list-scroll">
            {isLoadingSessions ? (
              <div className="empty-state">{t("chat.loadingSessions")}</div>
            ) : sessions.length > 0 ? (
              sessions.map((session) => (
                <div
                  key={session.id}
                  className={`session-item${session.id === selectedSessionId ? " is-selected" : ""}`}
                >
                  <button
                    type="button"
                    className="session-select"
                    onClick={() => {
                      selectSession(session.id);
                    }}
                  >
                    <span>{session.title}</span>
                    <span className="session-meta">{session.updatedAt}</span>
                  </button>
                  <button
                    type="button"
                    className="session-delete"
                    aria-label={`${t("chat.deleteSession")} ${session.title}`}
                    onClick={() => {
                      void deleteSession(session.id);
                    }}
                    disabled={deletingSessionId === session.id}
                  >
                    <Trash2 size={14} />
                  </button>
                </div>
              ))
            ) : (
              <div className="empty-state">{t("chat.noSessions")}</div>
            )}
          </div>
        </Panel>

        <Panel title={t("chat.title")} className="panel-chat-main" hideHeader>
          <div className="chat-main">
            <div className="chat-topline">
              <div>
                <div className="panel-eyebrow">{t("chat.eyebrow")}</div>
                <div className="chat-session-title">
                  {selectedSession?.title ?? t("chat.untitledSession")}
                </div>
              </div>
              <div className="chat-topline-meta">
                <span>{selectedSession?.updatedAt ?? t("chat.noHistory")}</span>
                <span
                  className="chat-status-dot"
                  title={t("status.connected")}
                  aria-label={t("status.connected")}
                />
              </div>
            </div>

            <div
              className="message-list"
              ref={messageListRef}
              onScroll={(event) => {
                const target = event.currentTarget;
                const distanceToBottom =
                  target.scrollHeight - target.scrollTop - target.clientHeight;
                shouldAutoScrollRef.current = distanceToBottom < 80;
              }}
            >
              {error && messages.length === 0 ? (
                <div className="empty-state">{error}</div>
              ) : null}
              {!error && isLoadingHistory && messages.length === 0 ? (
                <div className="empty-state">{t("chat.loadingHistory")}</div>
              ) : null}
              {!error && !isLoadingHistory && messages.length === 0 ? (
                <div className="empty-state">{t("chat.emptyState")}</div>
              ) : null}
              {error && messages.length > 0 ? (
                <div className="empty-state" style={{ marginBottom: "1rem" }}>
                  {error}
                </div>
              ) : null}
              {messages.map((message) => (
                  <article
                    key={message.id}
                    className={`message-bubble message-bubble-${message.role}`}
                  >
                    <div className="message-role">{message.role}</div>
                    {isSubmitting &&
                    message.role === "assistant" &&
                    message.id === pendingAssistantId ? (
                      <div className="chat-loading-inline" aria-live="polite">
                        <span>
                          {loadingLabelBase}
                          <span className="chat-loading-ellipsis" aria-hidden="true" />
                        </span>
                      </div>
                    ) : null}
                    {message.content ? (
                      <div className="message-content">
                        {renderMessageContent(message.content)}
                      </div>
                    ) : null}
                  </article>
                ))}
            </div>

            {activeTools.length > 0 ? (
              <div className="chat-stream-tools">
                {activeTools.map((tool) => (
                  <div key={tool.toolId} className={`chat-tool-chip chat-tool-chip-${tool.status}`}>
                    <span className="chat-tool-chip-label">{tool.label}</span>
                    <strong>{t(`chat.toolStatus.${tool.status}`)}</strong>
                  </div>
                ))}
              </div>
            ) : null}

            <form
              className="composer composer-inline"
              onSubmit={(event) => {
                event.preventDefault();
                void handleSubmit();
              }}
            >
              <div className="composer-shell">
                <textarea
                  className="composer-input"
                  rows={3}
                  placeholder={t("chat.inputPlaceholder")}
                  value={composerText}
                  onChange={(event) => {
                    setComposerText(event.target.value);
                  }}
                  onKeyDown={(event) => {
                    if (
                      event.key === "Enter" &&
                      !event.shiftKey &&
                      !event.nativeEvent.isComposing
                    ) {
                      event.preventDefault();
                      void handleSubmit();
                    }
                  }}
                  disabled={isSubmitting || !canAccessProtectedApi}
                />
                {deletingSessionId ? (
                  <div className="composer-hint">{t("chat.deleting")}</div>
                ) : null}
                <button
                  type="submit"
                  className="composer-submit"
                  disabled={
                    isSubmitting || !composerText.trim() || !canAccessProtectedApi
                  }
                >
                  <SendHorizontal size={16} />
                  <span className="sr-only">{isSubmitting ? "Sending..." : t("chat.send")}</span>
                </button>
              </div>
            </form>
          </div>
        </Panel>

        <Panel eyebrow={t("chat.panels.inspector")} title={t("status.local")}>
            <div className="metric-grid metric-grid-scroll">
              <div className="metric-card">
                <span className="metric-label">{t("status.providerReady")}</span>
                <strong>{selectedSession ? t("chat.inspector.sessionLoaded") : t("chat.inspector.waitingForSession")}</strong>
              </div>
              <div className="metric-card">
                <span className="metric-label">{t("status.memoryHealthy")}</span>
                <strong>{messages.length > 0 ? `${messages.length} messages` : "No history"}</strong>
              </div>
              <div className="metric-card">
                <span className="metric-label">{t("chat.memoryWindow.label")}</span>
                <strong>
                  {memoryWindow !== null
                    ? t("chat.memoryWindow.value", { count: memoryWindow })
                    : t("chat.memoryWindow.pending")}
                </strong>
              </div>
              <div className="metric-card">
                <span className="metric-label">{t("chat.currentModel.label")}</span>
                <strong title={currentModel || t("chat.currentModel.pending")}>
                  {currentModel || t("chat.currentModel.pending")}
                </strong>
                <span>
                  {currentProvider
                    ? t("chat.currentModel.provider", { provider: currentProvider })
                    : t("chat.currentModel.providerPending")}
                </span>
              </div>
            </div>
          </Panel>
        </div>
    </div>
  );
}
