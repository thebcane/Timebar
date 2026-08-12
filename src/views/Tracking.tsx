import { useCallback, useEffect, useMemo, useState } from "react";
import { Plus, Share } from "lucide-react";
import { useStore } from "../store/store";
import * as ipc from "../lib/ipc";
import { filterTree } from "../lib/tree";
import { getTz, rangeLastNDays } from "../lib/time";
import type { DayGroup, Folder, Task, TreeNode } from "../lib/types";
import { TaskTree } from "../components/TaskTree";
import { EntriesFeed } from "../components/EntriesFeed";
import { TaskForm } from "../components/TaskForm";
import { EntryForm } from "../components/EntryForm";
import { Eyebrow } from "../components/primitives";

type Tab = "all" | "recent" | "archive";
const TABS: { key: Tab; label: string }[] = [
  { key: "all", label: "All Tasks" },
  { key: "recent", label: "Recent" },
  { key: "archive", label: "Archive" },
];

export default function Tracking() {
  const active = useStore((s) => s.activeSession);
  const toggle = useStore((s) => s.toggle);
  const bump = useStore((s) => s.bump);
  const requestConfirm = useStore((s) => s.requestConfirm);
  const notify = useStore((s) => s.notify);
  const dataVersion = useStore((s) => s.dataVersion);

  const [tab, setTab] = useState<Tab>("all");
  const [tree, setTree] = useState<TreeNode[]>([]);
  const [days, setDays] = useState<DayGroup[]>([]);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [folders, setFolders] = useState<Folder[]>([]);

  const [taskForm, setTaskForm] = useState<{ open: boolean; task?: Task }>({ open: false });
  const [entryForm, setEntryForm] = useState<{ open: boolean; sessionId?: string }>({ open: false });
  const [loadedAt, setLoadedAt] = useState(Date.now());

  const load = useCallback(async () => {
    const tz = getTz();
    // Recompute the range each load so it stays current across day changes.
    const range = rangeLastNDays(30);
    const [fullTree, entries, allTasks, allFolders] = await Promise.all([
      ipc.getTaskTreeWithTotals(range, tz, true),
      ipc.getTimeEntries(range, tz),
      ipc.listTasks(true),
      ipc.listFolders(),
    ]);
    setTree(fullTree);
    setDays(entries);
    setTasks(allTasks);
    setFolders(allFolders);
    setLoadedAt(Date.now());
  }, []);

  useEffect(() => {
    load();
  }, [load, dataVersion, active?.id]);

  const visibleTree = useMemo(() => {
    if (tab === "archive") {
      return filterTree(
        tree,
        (t) => t.archived,
        (f, kids) => f.archived || kids.length > 0,
      );
    }
    const nonArchived = filterTree(
      tree,
      (t) => !t.archived,
      // Keep non-archived folders even when empty (e.g. a just-created one).
      (f) => !f.archived,
    );
    if (tab === "recent") {
      return filterTree(
        nonArchived,
        (t) => t.totalMs > 0,
        (_f, kids) => kids.length > 0,
      );
    }
    return nonArchived;
  }, [tree, tab]);

  const taskById = useMemo(() => new Map(tasks.map((t) => [t.id, t])), [tasks]);

  const onMoveTask = async (taskId: string, folderId: string | null) => {
    await ipc.updateTask(taskId, { folderId });
    bump();
  };
  const onMoveFolder = async (folderId: string, parentId: string | null) => {
    await ipc.updateFolder(folderId, { parentId });
    bump();
  };
  const onReorder = async (entity: "task" | "folder", orderedIds: string[]) => {
    await ipc.reorder(entity, orderedIds);
    bump();
  };
  const onArchive = async (node: TreeNode) => {
    if (node.kind === "task") await ipc.archiveTask(node.id, !node.archived);
    else await ipc.archiveFolder(node.id, !node.archived);
    bump();
  };
  const onDelete = async (node: TreeNode) => {
    const msg =
      node.kind === "task"
        ? `Delete "${node.name}" and all its sessions? This cannot be undone.`
        : `Delete folder "${node.name}"? Its tasks move to the top level.`;
    if (!(await requestConfirm(msg))) return;
    if (node.kind === "task") await ipc.deleteTask(node.id);
    else await ipc.deleteFolder(node.id);
    bump();
  };

  const exportCsv = async () => {
    const path = await ipc.exportCsv(rangeLastNDays(30), getTz());
    notify(`Exported to:\n${path}`);
  };

  return (
    <div className="flex h-full">
      {/* Left — task tree */}
      <div className="flex w-[380px] shrink-0 flex-col border-r border-hairline">
        <div className="flex items-center gap-2 px-4 pb-2 pt-4">
          <div className="flex flex-1 gap-1 rounded-pill bg-panel-raised p-1">
            {TABS.map((t) => (
              <button
                key={t.key}
                type="button"
                onClick={() => setTab(t.key)}
                className={`flex-1 rounded-pill px-2 py-1 text-[12px] font-medium transition-colors ${
                  tab === t.key ? "bg-panel text-text-primary" : "text-text-secondary"
                }`}
              >
                {t.label}
              </button>
            ))}
          </div>
          <button
            type="button"
            title="Add task or folder"
            onClick={() => setTaskForm({ open: true })}
            className="grid h-8 w-8 shrink-0 place-items-center rounded-full bg-panel-raised text-text-secondary hover:text-text-primary"
          >
            <Plus size={16} />
          </button>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-4">
          <TaskTree
            nodes={visibleTree}
            activeTaskId={active?.taskId}
            loadedAt={loadedAt}
            onToggle={toggle}
            onEditTask={(id) => {
              const t = taskById.get(id);
              if (t) setTaskForm({ open: true, task: t });
            }}
            onArchive={onArchive}
            onDelete={onDelete}
            onMoveTask={onMoveTask}
            onMoveFolder={onMoveFolder}
            onReorder={onReorder}
          />
        </div>
      </div>

      {/* Right — time entries */}
      <div className="flex min-w-0 flex-1 flex-col">
        <div className="flex items-center justify-between px-6 pb-2 pt-4">
          <div>
            <h1 className="text-[16px] font-semibold text-text-primary">Time Entries</h1>
            <Eyebrow>Last 30 days</Eyebrow>
          </div>
          <div className="flex items-center gap-2">
            <button
              type="button"
              title="Add manual entry"
              onClick={() => setEntryForm({ open: true })}
              className="grid h-8 w-8 place-items-center rounded-full bg-panel-raised text-text-secondary hover:text-text-primary"
            >
              <Plus size={16} />
            </button>
            <button
              type="button"
              title="Export CSV"
              onClick={exportCsv}
              className="grid h-8 w-8 place-items-center rounded-full bg-panel-raised text-text-secondary hover:text-text-primary"
            >
              <Share size={15} />
            </button>
          </div>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto px-4 pb-6">
          <EntriesFeed
            days={days}
            onEntryClick={(sid) => setEntryForm({ open: true, sessionId: sid })}
            onEntryDelete={async (sid) => {
              if (!(await requestConfirm("Delete this entry?"))) return;
              await ipc.deleteSession(sid);
              bump();
            }}
          />
        </div>
      </div>

      {taskForm.open ? (
        <TaskForm
          folders={folders}
          task={taskForm.task}
          onClose={() => setTaskForm({ open: false })}
          onSaved={bump}
        />
      ) : null}
      {entryForm.open ? (
        <EntryForm
          tasks={tasks.filter((t) => !t.archived)}
          sessionId={entryForm.sessionId}
          onClose={() => setEntryForm({ open: false })}
          onSaved={bump}
        />
      ) : null}
    </div>
  );
}
