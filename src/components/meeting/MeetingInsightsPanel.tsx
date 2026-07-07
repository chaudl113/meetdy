import React, { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Sparkles,
  Loader2,
  ChevronDown,
  ChevronUp,
  RefreshCw,
  Plus,
  Trash2,
  CheckSquare,
  Square,
  ListChecks,
  Users,
  ListTree,
  Tag as TagIcon,
  X,
} from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import {
  commands,
  type ActionItem,
  type ActionItemStatus,
  type KeyPoint,
  type MeetingInsights,
  type Participant,
  type Tag,
} from "@/bindings";

interface MeetingInsightsPanelProps {
  sessionId: string;
  hasTranscript: boolean;
}

const STATUS_CYCLE: ActionItemStatus[] = [
  "todo",
  "in_progress",
  "done",
  "blocked",
];

function nextStatus(current: ActionItemStatus): ActionItemStatus {
  const idx = STATUS_CYCLE.indexOf(current);
  return STATUS_CYCLE[(idx + 1) % STATUS_CYCLE.length];
}

function statusBadgeClass(status: ActionItemStatus): string {
  switch (status) {
    case "done":
      return "bg-green-500/20 text-green-300 border-green-500/30";
    case "in_progress":
      return "bg-blue-500/20 text-blue-300 border-blue-500/30";
    case "blocked":
      return "bg-red-500/20 text-red-300 border-red-500/30";
    case "todo":
    default:
      return "bg-mid-gray/20 text-mid-gray border-mid-gray/30";
  }
}

/**
 * MeetingInsightsPanel - AI-extracted structured insights for a meeting:
 * key points, action items, participants, and tags. Supports manual CRUD
 * and one-shot LLM extraction that replaces previous AI data.
 */
export const MeetingInsightsPanel: React.FC<MeetingInsightsPanelProps> = ({
  sessionId,
  hasTranscript,
}) => {
  const { t } = useTranslation();

  const [isExpanded, setIsExpanded] = useState(true);
  const [loading, setLoading] = useState(true);
  const [isExtracting, setIsExtracting] = useState(false);
  const [statusMessage, setStatusMessage] = useState("");
  const [pullPercent, setPullPercent] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  const [keyPoints, setKeyPoints] = useState<KeyPoint[]>([]);
  const [actionItems, setActionItems] = useState<ActionItem[]>([]);
  const [participants, setParticipants] = useState<Participant[]>([]);
  const [tags, setTags] = useState<Tag[]>([]);

  // Inline add forms
  const [newKeyPoint, setNewKeyPoint] = useState("");
  const [newKeyPointCategory, setNewKeyPointCategory] = useState("");
  const [newTask, setNewTask] = useState("");
  const [newAssignee, setNewAssignee] = useState("");
  const [newDueDate, setNewDueDate] = useState("");
  const [newParticipantName, setNewParticipantName] = useState("");
  const [newParticipantRole, setNewParticipantRole] = useState("");
  const [newTagLabel, setNewTagLabel] = useState("");

  const unlistenRef = useRef<Array<() => void>>([]);

  // Initial load
  useEffect(() => {
    const load = async () => {
      setLoading(true);
      setError(null);
      try {
        const [kp, ai, ps, tg] = await Promise.all([
          commands.listMeetingKeyPoints(sessionId),
          commands.listMeetingActionItems(sessionId),
          commands.listMeetingParticipants(sessionId),
          commands.listMeetingTags(sessionId),
        ]);
        if (kp.status === "ok") setKeyPoints(kp.data);
        if (ai.status === "ok") setActionItems(ai.data);
        if (ps.status === "ok") setParticipants(ps.data);
        if (tg.status === "ok") setTags(tg.data);
      } catch (e) {
        console.error("Failed to load insights:", e);
      } finally {
        setLoading(false);
      }
    };
    load();
  }, [sessionId]);

  // Cleanup listeners on unmount
  useEffect(() => {
    return () => {
      for (const u of unlistenRef.current) u();
      unlistenRef.current = [];
    };
  }, []);

  const applyInsights = useCallback((insights: MeetingInsights) => {
    setKeyPoints(insights.key_points);
    setActionItems(insights.action_items);
    setParticipants(insights.participants);
    setTags(insights.tags);
  }, []);

  const handleExtract = async () => {
    if (!hasTranscript) return;
    setIsExtracting(true);
    setError(null);
    setStatusMessage(
      t("meeting.insights.extracting", "Extracting insights..."),
    );
    setPullPercent(null);

    const cleanups: Array<() => void> = [];
    try {
      const unlistenStatus = await listen<string>(
        "meeting_insights_status",
        (event) => setStatusMessage(event.payload),
      );
      cleanups.push(unlistenStatus);

      const unlistenPull = await listen<{
        model: string;
        status: string;
        percent: number | null;
      }>("ollama_pull_progress", (event) => {
        const { status, percent } = event.payload;
        if (status === "success") {
          setStatusMessage(
            t("meeting.insights.extracting", "Extracting insights..."),
          );
          setPullPercent(null);
        } else {
          const pct = percent != null ? ` (${Math.round(percent)}%)` : "";
          setStatusMessage(
            `${t("meeting.insights.downloadingModel", "Downloading model")}${pct}`,
          );
          setPullPercent(percent);
        }
      });
      cleanups.push(unlistenPull);

      const result = await commands.extractMeetingInsights(sessionId, null);
      if (result.status === "ok") {
        applyInsights(result.data);
      } else {
        setError(result.error);
      }
    } catch (e) {
      setError(
        e instanceof Error
          ? e.message
          : t("meeting.insights.extractError", "Failed to extract insights"),
      );
    } finally {
      setIsExtracting(false);
      setStatusMessage("");
      setPullPercent(null);
      for (const c of cleanups) c();
    }
  };

  // --- Key Points ---------------------------------------------------------

  const handleAddKeyPoint = async () => {
    const content = newKeyPoint.trim();
    if (!content) return;
    const category = newKeyPointCategory.trim() || null;
    const result = await commands.addMeetingKeyPoint(
      sessionId,
      category,
      content,
    );
    if (result.status === "ok") {
      setKeyPoints((prev) => [...prev, result.data]);
      setNewKeyPoint("");
      setNewKeyPointCategory("");
    } else {
      setError(result.error);
    }
  };

  // No delete endpoint for key points individually; re-extracting will replace.

  // --- Action Items -------------------------------------------------------

  const handleAddActionItem = async () => {
    const task = newTask.trim();
    if (!task) return;
    const result = await commands.addMeetingActionItem(
      sessionId,
      task,
      newAssignee.trim() || null,
      newDueDate.trim() || null,
      null,
    );
    if (result.status === "ok") {
      setActionItems((prev) => [...prev, result.data]);
      setNewTask("");
      setNewAssignee("");
      setNewDueDate("");
    } else {
      setError(result.error);
    }
  };

  const handleToggleStatus = async (item: ActionItem) => {
    const updated = { ...item, status: nextStatus(item.status) };
    setActionItems((prev) => prev.map((i) => (i.id === item.id ? updated : i)));
    const result = await commands.updateMeetingActionItem(updated);
    if (result.status !== "ok") {
      // revert
      setActionItems((prev) => prev.map((i) => (i.id === item.id ? item : i)));
      setError(result.error);
    }
  };

  const handleDeleteActionItem = async (id: string) => {
    const prev = actionItems;
    setActionItems((p) => p.filter((i) => i.id !== id));
    const result = await commands.deleteMeetingActionItem(id);
    if (result.status !== "ok") {
      setActionItems(prev);
      setError(result.error);
    }
  };

  // --- Participants -------------------------------------------------------

  const handleAddParticipant = async () => {
    const name = newParticipantName.trim();
    if (!name) return;
    const result = await commands.addMeetingParticipant(
      sessionId,
      name,
      newParticipantRole.trim() || null,
    );
    if (result.status === "ok") {
      setParticipants((prev) => [...prev, result.data]);
      setNewParticipantName("");
      setNewParticipantRole("");
    } else {
      setError(result.error);
    }
  };

  const handleDeleteParticipant = async (id: string) => {
    const prev = participants;
    setParticipants((p) => p.filter((x) => x.id !== id));
    const result = await commands.deleteMeetingParticipant(id);
    if (result.status !== "ok") {
      setParticipants(prev);
      setError(result.error);
    }
  };

  // --- Tags ---------------------------------------------------------------

  const handleAddTag = async () => {
    const label = newTagLabel.trim();
    if (!label) return;
    const result = await commands.addMeetingTag(sessionId, label, null);
    if (result.status === "ok") {
      setTags((prev) => [...prev, result.data]);
      setNewTagLabel("");
    } else {
      setError(result.error);
    }
  };

  const handleDeleteTag = async (id: string) => {
    const prev = tags;
    setTags((p) => p.filter((x) => x.id !== id));
    const result = await commands.deleteMeetingTag(id);
    if (result.status !== "ok") {
      setTags(prev);
      setError(result.error);
    }
  };

  // Group key points by category for nicer display
  const groupedKeyPoints = React.useMemo(() => {
    const groups = new Map<string, KeyPoint[]>();
    for (const kp of keyPoints) {
      const key = kp.category || "";
      if (!groups.has(key)) groups.set(key, []);
      groups.get(key)!.push(kp);
    }
    return Array.from(groups.entries());
  }, [keyPoints]);

  const hasAnyInsights =
    keyPoints.length > 0 ||
    actionItems.length > 0 ||
    participants.length > 0 ||
    tags.length > 0;

  return (
    <div className="space-y-2">
      {/* Header */}
      <div className="flex items-center justify-between">
        <button
          type="button"
          onClick={() => setIsExpanded(!isExpanded)}
          aria-expanded={isExpanded}
          aria-controls="meeting-insights-content"
          className="flex items-center gap-2 text-sm font-medium text-mid-gray hover:text-white transition-colors"
        >
          <Sparkles className="h-4 w-4 text-amber-400" aria-hidden="true" />
          {t("meeting.insights.title", "Insights")}
          {isExpanded ? (
            <ChevronUp className="h-4 w-4" aria-hidden="true" />
          ) : (
            <ChevronDown className="h-4 w-4" aria-hidden="true" />
          )}
        </button>

        {hasAnyInsights && hasTranscript && !isExtracting && (
          <button
            type="button"
            onClick={handleExtract}
            className="inline-flex items-center gap-1.5 px-2 py-1 text-xs text-mid-gray hover:text-white hover:bg-mid-gray/20 rounded transition-colors"
            aria-label={t("meeting.insights.reextract", "Re-extract")}
            title={t(
              "meeting.insights.reextractHint",
              "Re-extracts key points, action items and participants. Tags are preserved.",
            )}
          >
            <RefreshCw className="h-3.5 w-3.5" aria-hidden="true" />
          </button>
        )}
      </div>

      {isExpanded && (
        <div
          id="meeting-insights-content"
          className="bg-dark-gray/30 rounded-lg p-4 space-y-5"
        >
          {error && (
            <div className="p-2 bg-red-500/10 border border-red-500/30 rounded text-sm text-red-400 flex items-start justify-between gap-2">
              <span>{error}</span>
              <button
                onClick={() => setError(null)}
                className="text-red-300 hover:text-red-100"
              >
                <X className="h-3.5 w-3.5" />
              </button>
            </div>
          )}

          {isExtracting ? (
            <div className="flex flex-col items-center justify-center py-8 text-mid-gray gap-3">
              <div className="flex items-center">
                <Loader2 className="h-5 w-5 animate-spin mr-2" />
                <span className="text-sm">
                  {statusMessage ||
                    t("meeting.insights.extracting", "Extracting insights...")}
                </span>
              </div>
              {pullPercent != null && (
                <div className="w-full max-w-xs">
                  <div className="h-1.5 bg-mid-gray/20 rounded-full overflow-hidden">
                    <div
                      className="h-full bg-amber-500 rounded-full transition-all duration-300"
                      style={{ width: `${Math.min(pullPercent, 100)}%` }}
                    />
                  </div>
                  <p className="text-xs text-mid-gray/70 text-center mt-1">
                    {Math.round(pullPercent)}%
                  </p>
                </div>
              )}
            </div>
          ) : loading ? (
            <div className="text-sm text-mid-gray text-center py-4">
              {t("common.loading", "Loading...")}
            </div>
          ) : (
            <>
              {/* Empty state with Extract CTA */}
              {!hasAnyInsights && (
                <div className="flex flex-col items-center justify-center py-6 text-center">
                  <Sparkles className="h-8 w-8 text-amber-400/50 mb-3" />
                  <p className="text-sm text-mid-gray mb-4">
                    {hasTranscript
                      ? t(
                          "meeting.insights.empty",
                          "No insights yet. Extract structured data from the transcript or add items manually.",
                        )
                      : t(
                          "meeting.insights.noTranscript",
                          "Transcript required to extract insights. You can still add items manually below.",
                        )}
                  </p>
                  {hasTranscript && (
                    <button
                      type="button"
                      onClick={handleExtract}
                      className="inline-flex items-center gap-2 px-4 py-2 bg-amber-600 hover:bg-amber-700 text-white rounded-lg transition-colors"
                    >
                      <Sparkles className="h-4 w-4" />
                      {t("meeting.insights.extract", "Extract Insights")}
                    </button>
                  )}
                </div>
              )}

              {/* Tags */}
              <section className="space-y-2">
                <h4 className="text-xs font-semibold uppercase tracking-wide text-mid-gray flex items-center gap-1.5">
                  <TagIcon className="h-3.5 w-3.5" />
                  {t("meeting.insights.tags", "Tags")}
                </h4>
                <div className="flex flex-wrap gap-1.5">
                  {tags.map((tag) => (
                    <span
                      key={tag.id}
                      className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-mid-gray/20 text-xs text-white border border-mid-gray/30"
                    >
                      {tag.label}
                      <button
                        type="button"
                        onClick={() => handleDeleteTag(tag.id)}
                        className="hover:text-red-300"
                        aria-label={t("common.remove", "Remove")}
                      >
                        <X className="h-3 w-3" />
                      </button>
                    </span>
                  ))}
                  <form
                    onSubmit={(e) => {
                      e.preventDefault();
                      handleAddTag();
                    }}
                    className="inline-flex items-center"
                  >
                    <input
                      type="text"
                      value={newTagLabel}
                      onChange={(e) => setNewTagLabel(e.target.value)}
                      placeholder={t(
                        "meeting.insights.addTag",
                        "Add tag…",
                      )}
                      className="bg-dark-gray/50 border border-mid-gray/30 rounded-full px-2.5 py-0.5 text-xs text-white placeholder-mid-gray/60 focus:outline-none focus:border-amber-500 w-28"
                    />
                  </form>
                </div>
              </section>

              {/* Participants */}
              <section className="space-y-2">
                <h4 className="text-xs font-semibold uppercase tracking-wide text-mid-gray flex items-center gap-1.5">
                  <Users className="h-3.5 w-3.5" />
                  {t("meeting.insights.participants", "Participants")}
                  <span className="text-mid-gray/60 normal-case">
                    ({participants.length})
                  </span>
                </h4>
                {participants.length > 0 && (
                  <ul className="space-y-1">
                    {participants.map((p) => (
                      <li
                        key={p.id}
                        className="flex items-center justify-between gap-2 text-sm bg-dark-gray/40 rounded px-2 py-1"
                      >
                        <div className="min-w-0 flex-1">
                          <span className="text-white">{p.name}</span>
                          {p.role && (
                            <span className="text-mid-gray ml-2">
                              · {p.role}
                            </span>
                          )}
                        </div>
                        <button
                          type="button"
                          onClick={() => handleDeleteParticipant(p.id)}
                          className="text-mid-gray hover:text-red-400 flex-shrink-0"
                          aria-label={t("common.delete", "Delete")}
                        >
                          <Trash2 className="h-3.5 w-3.5" />
                        </button>
                      </li>
                    ))}
                  </ul>
                )}
                <form
                  onSubmit={(e) => {
                    e.preventDefault();
                    handleAddParticipant();
                  }}
                  className="flex gap-1.5"
                >
                  <input
                    type="text"
                    value={newParticipantName}
                    onChange={(e) => setNewParticipantName(e.target.value)}
                    placeholder={t(
                      "meeting.insights.participantName",
                      "Name",
                    )}
                    className="flex-1 min-w-0 bg-dark-gray/50 border border-mid-gray/30 rounded px-2 py-1 text-xs text-white placeholder-mid-gray/60 focus:outline-none focus:border-amber-500"
                  />
                  <input
                    type="text"
                    value={newParticipantRole}
                    onChange={(e) => setNewParticipantRole(e.target.value)}
                    placeholder={t(
                      "meeting.insights.participantRole",
                      "Role (optional)",
                    )}
                    className="flex-1 min-w-0 bg-dark-gray/50 border border-mid-gray/30 rounded px-2 py-1 text-xs text-white placeholder-mid-gray/60 focus:outline-none focus:border-amber-500"
                  />
                  <button
                    type="submit"
                    className="px-2 py-1 bg-mid-gray/20 hover:bg-mid-gray/30 text-white rounded"
                    aria-label={t("common.add", "Add")}
                  >
                    <Plus className="h-3.5 w-3.5" />
                  </button>
                </form>
              </section>

              {/* Key Points */}
              <section className="space-y-2">
                <h4 className="text-xs font-semibold uppercase tracking-wide text-mid-gray flex items-center gap-1.5">
                  <ListTree className="h-3.5 w-3.5" />
                  {t("meeting.insights.keyPoints", "Key Points")}
                  <span className="text-mid-gray/60 normal-case">
                    ({keyPoints.length})
                  </span>
                </h4>
                {groupedKeyPoints.length > 0 && (
                  <div className="space-y-3">
                    {groupedKeyPoints.map(([category, items]) => (
                      <div key={category || "_uncategorized"}>
                        {category && (
                          <h5 className="text-xs font-medium text-white/80 mb-1">
                            {category}
                          </h5>
                        )}
                        <ul className="space-y-1 list-disc list-inside text-sm text-white/90 marker:text-mid-gray">
                          {items.map((kp) => (
                            <li key={kp.id} className="leading-snug">
                              {kp.content}
                            </li>
                          ))}
                        </ul>
                      </div>
                    ))}
                  </div>
                )}
                <form
                  onSubmit={(e) => {
                    e.preventDefault();
                    handleAddKeyPoint();
                  }}
                  className="flex gap-1.5"
                >
                  <input
                    type="text"
                    value={newKeyPointCategory}
                    onChange={(e) => setNewKeyPointCategory(e.target.value)}
                    placeholder={t(
                      "meeting.insights.keyPointCategory",
                      "Category (optional)",
                    )}
                    className="w-32 flex-shrink-0 bg-dark-gray/50 border border-mid-gray/30 rounded px-2 py-1 text-xs text-white placeholder-mid-gray/60 focus:outline-none focus:border-amber-500"
                  />
                  <input
                    type="text"
                    value={newKeyPoint}
                    onChange={(e) => setNewKeyPoint(e.target.value)}
                    placeholder={t(
                      "meeting.insights.addKeyPoint",
                      "Add a key point…",
                    )}
                    className="flex-1 min-w-0 bg-dark-gray/50 border border-mid-gray/30 rounded px-2 py-1 text-xs text-white placeholder-mid-gray/60 focus:outline-none focus:border-amber-500"
                  />
                  <button
                    type="submit"
                    className="px-2 py-1 bg-mid-gray/20 hover:bg-mid-gray/30 text-white rounded"
                    aria-label={t("common.add", "Add")}
                  >
                    <Plus className="h-3.5 w-3.5" />
                  </button>
                </form>
              </section>

              {/* Action Items */}
              <section className="space-y-2">
                <h4 className="text-xs font-semibold uppercase tracking-wide text-mid-gray flex items-center gap-1.5">
                  <ListChecks className="h-3.5 w-3.5" />
                  {t("meeting.insights.actionItems", "Action Items")}
                  <span className="text-mid-gray/60 normal-case">
                    ({actionItems.length})
                  </span>
                </h4>
                {actionItems.length > 0 && (
                  <ul className="space-y-1.5">
                    {actionItems.map((item) => {
                      const isDone = item.status === "done";
                      return (
                        <li
                          key={item.id}
                          className="flex items-start gap-2 bg-dark-gray/40 rounded px-2 py-1.5 text-sm"
                        >
                          <button
                            type="button"
                            onClick={() => handleToggleStatus(item)}
                            className="text-mid-gray hover:text-white mt-0.5 flex-shrink-0"
                            aria-label={t(
                              "meeting.insights.cycleStatus",
                              "Cycle status",
                            )}
                            title={item.status}
                          >
                            {isDone ? (
                              <CheckSquare className="h-4 w-4 text-green-400" />
                            ) : (
                              <Square className="h-4 w-4" />
                            )}
                          </button>
                          <div className="min-w-0 flex-1">
                            <div
                              className={`${isDone ? "line-through text-mid-gray" : "text-white"}`}
                            >
                              {item.task}
                            </div>
                            <div className="flex flex-wrap items-center gap-1.5 mt-1 text-xs">
                              <span
                                className={`px-1.5 py-0.5 rounded border ${statusBadgeClass(item.status)}`}
                              >
                                {item.status.replace("_", " ")}
                              </span>
                              {item.assignee && (
                                <span className="text-mid-gray">
                                  @{item.assignee}
                                </span>
                              )}
                              {item.due_date && (
                                <span className="text-mid-gray">
                                  {`⏷ ${item.due_date}`}
                                </span>
                              )}
                            </div>
                          </div>
                          <button
                            type="button"
                            onClick={() => handleDeleteActionItem(item.id)}
                            className="text-mid-gray hover:text-red-400 flex-shrink-0 mt-0.5"
                            aria-label={t("common.delete", "Delete")}
                          >
                            <Trash2 className="h-3.5 w-3.5" />
                          </button>
                        </li>
                      );
                    })}
                  </ul>
                )}
                <form
                  onSubmit={(e) => {
                    e.preventDefault();
                    handleAddActionItem();
                  }}
                  className="space-y-1.5"
                >
                  <input
                    type="text"
                    value={newTask}
                    onChange={(e) => setNewTask(e.target.value)}
                    placeholder={t(
                      "meeting.insights.addTask",
                      "Add a task…",
                    )}
                    className="w-full bg-dark-gray/50 border border-mid-gray/30 rounded px-2 py-1 text-xs text-white placeholder-mid-gray/60 focus:outline-none focus:border-amber-500"
                  />
                  <div className="flex gap-1.5">
                    <input
                      type="text"
                      value={newAssignee}
                      onChange={(e) => setNewAssignee(e.target.value)}
                      placeholder={t(
                        "meeting.insights.assignee",
                        "Assignee",
                      )}
                      className="flex-1 min-w-0 bg-dark-gray/50 border border-mid-gray/30 rounded px-2 py-1 text-xs text-white placeholder-mid-gray/60 focus:outline-none focus:border-amber-500"
                    />
                    <input
                      type="text"
                      value={newDueDate}
                      onChange={(e) => setNewDueDate(e.target.value)}
                      placeholder={t(
                        "meeting.insights.dueDate",
                        "Due date",
                      )}
                      className="flex-1 min-w-0 bg-dark-gray/50 border border-mid-gray/30 rounded px-2 py-1 text-xs text-white placeholder-mid-gray/60 focus:outline-none focus:border-amber-500"
                    />
                    <button
                      type="submit"
                      className="px-2 py-1 bg-mid-gray/20 hover:bg-mid-gray/30 text-white rounded"
                      aria-label={t("common.add", "Add")}
                    >
                      <Plus className="h-3.5 w-3.5" />
                    </button>
                  </div>
                </form>
              </section>
            </>
          )}
        </div>
      )}
    </div>
  );
};

export default MeetingInsightsPanel;
