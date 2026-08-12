import { useEffect, useState } from "react";
import { Trash2 } from "lucide-react";
import { Modal, Field, TextInput, Select, Toggle, Btn } from "./ui";
import * as ipc from "../lib/ipc";
import { useStore } from "../store/store";
import { toDatetimeLocal, fromDatetimeLocal } from "../lib/time";
import type { Task } from "../lib/types";

/**
 * Create a manual entry or edit an existing session. Editing loads the whole
 * (unclamped) session so midnight-split display never corrupts stored times.
 */
export function EntryForm({
  tasks,
  sessionId,
  defaultTaskId,
  onClose,
  onSaved,
}: {
  tasks: Task[];
  sessionId?: string;
  defaultTaskId?: string;
  onClose: () => void;
  onSaved: () => void;
}) {
  const editing = !!sessionId;
  const requestConfirm = useStore((s) => s.requestConfirm);
  const now = Date.now();

  const [taskId, setTaskId] = useState(defaultTaskId ?? tasks[0]?.id ?? "");
  const [start, setStart] = useState(toDatetimeLocal(now - 3_600_000));
  const [end, setEnd] = useState(toDatetimeLocal(now));
  const [note, setNote] = useState("");
  const [billed, setBilled] = useState(false);
  const [resnapshot, setResnapshot] = useState(false);
  const [origTaskId, setOrigTaskId] = useState(defaultTaskId ?? "");
  const [error, setError] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(!editing);

  useEffect(() => {
    if (!sessionId) return;
    ipc.getSession(sessionId).then((s) => {
      if (!s) return;
      setTaskId(s.taskId);
      setOrigTaskId(s.taskId);
      setStart(toDatetimeLocal(s.startedAt));
      setEnd(toDatetimeLocal(s.endedAt ?? Date.now()));
      setNote(s.note ?? "");
      setBilled(s.billed);
      setLoaded(true);
    });
  }, [sessionId]);

  const save = async () => {
    const s = fromDatetimeLocal(start);
    const e = fromDatetimeLocal(end);
    if (!taskId) return setError("Pick a task.");
    if (!(e > s)) return setError("End must be after start.");
    try {
      if (editing) {
        await ipc.updateSession(sessionId!, {
          taskId,
          startedAt: s,
          endedAt: e,
          note: note || null,
          billed,
          resnapshotRate: resnapshot && taskId !== origTaskId,
        });
      } else {
        await ipc.createManualSession({ taskId, startedAt: s, endedAt: e, note: note || null });
      }
      onSaved();
      onClose();
    } catch (err) {
      setError(String(err));
    }
  };

  const remove = async () => {
    if (!sessionId) return;
    if (!(await requestConfirm("Delete this entry?"))) return;
    await ipc.deleteSession(sessionId);
    onSaved();
    onClose();
  };

  if (!loaded) return null;

  return (
    <Modal title={editing ? "Edit entry" : "New entry"} onClose={onClose}>
      <Field label="Task">
        <Select value={taskId} onChange={(e) => setTaskId(e.target.value)}>
          {tasks.map((t) => (
            <option key={t.id} value={t.id}>
              {t.name}
            </option>
          ))}
        </Select>
      </Field>
      <div className="flex gap-3">
        <div className="flex-1">
          <Field label="Start">
            <TextInput type="datetime-local" value={start} onChange={(e) => setStart(e.target.value)} />
          </Field>
        </div>
        <div className="flex-1">
          <Field label="End">
            <TextInput type="datetime-local" value={end} onChange={(e) => setEnd(e.target.value)} />
          </Field>
        </div>
      </div>
      <Field label="Note">
        <TextInput value={note} onChange={(e) => setNote(e.target.value)} placeholder="Optional" />
      </Field>

      {editing ? (
        <div className="mb-3 flex flex-col gap-2">
          <Toggle checked={billed} onChange={setBilled} label="Marked as billed / paid" />
          {taskId !== origTaskId ? (
            <Toggle checked={resnapshot} onChange={setResnapshot} label="Update rate to new task's rate" />
          ) : null}
        </div>
      ) : null}

      {error ? <div className="mb-3 text-[12px] text-task-pink">{error}</div> : null}

      <div className="mt-2 flex items-center justify-between">
        {editing ? (
          <button
            type="button"
            onClick={remove}
            className="flex items-center gap-1.5 text-[13px] text-task-pink hover:opacity-80"
          >
            <Trash2 size={15} /> Delete
          </button>
        ) : (
          <span />
        )}
        <div className="flex gap-2">
          <Btn variant="ghost" onClick={onClose}>
            Cancel
          </Btn>
          <Btn variant="primary" onClick={save}>
            {editing ? "Save" : "Add entry"}
          </Btn>
        </div>
      </div>
    </Modal>
  );
}
