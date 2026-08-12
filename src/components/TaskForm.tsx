import { useState } from "react";
import { Trash2 } from "lucide-react";
import { Modal, Field, TextInput, Select, Toggle, Btn, ColorPicker } from "./ui";
import * as ipc from "../lib/ipc";
import { useStore } from "../store/store";
import { PALETTE, DEFAULT_COLOR } from "../lib/colors";
import type { Folder, Task } from "../lib/types";

export function TaskForm({
  folders,
  task,
  onClose,
  onSaved,
}: {
  folders: Folder[];
  task?: Task;
  onClose: () => void;
  onSaved: () => void;
}) {
  const editing = !!task;
  const requestConfirm = useStore((s) => s.requestConfirm);
  const [kind, setKind] = useState<"task" | "folder">("task");
  const [name, setName] = useState(task?.name ?? "");
  const [color, setColor] = useState(task?.color ?? DEFAULT_COLOR);
  const [folderId, setFolderId] = useState(task?.folderId ?? "");
  const [rate, setRate] = useState(String(task?.hourlyRate ?? 0));
  const [currency, setCurrency] = useState(task?.currency ?? "USD");
  const [billable, setBillable] = useState(task?.billable ?? false);
  const [archived, setArchived] = useState(task?.archived ?? false);
  const [error, setError] = useState<string | null>(null);

  const isFolder = !editing && kind === "folder";

  const save = async () => {
    if (!name.trim()) return setError("Name is required.");
    try {
      if (isFolder) {
        await ipc.createFolder(name.trim(), color, folderId || null);
      } else if (editing) {
        await ipc.updateTask(task!.id, {
          name: name.trim(),
          color,
          folderId: folderId || null,
          hourlyRate: parseFloat(rate) || 0,
          currency,
          billable,
          archived,
        });
      } else {
        await ipc.createTask({
          name: name.trim(),
          color,
          folderId: folderId || null,
          hourlyRate: parseFloat(rate) || 0,
          currency,
          billable,
        });
      }
      onSaved();
      onClose();
    } catch (err) {
      setError(String(err));
    }
  };

  const remove = async () => {
    if (!task) return;
    if (!(await requestConfirm(`Delete "${task.name}" and all its sessions? This cannot be undone.`)))
      return;
    await ipc.deleteTask(task.id);
    onSaved();
    onClose();
  };

  return (
    <Modal title={editing ? "Edit task" : "New"} onClose={onClose}>
      {!editing ? (
        <div className="mb-4 flex gap-1 rounded-pill bg-panel-raised p-1">
          {(["task", "folder"] as const).map((k) => (
            <button
              key={k}
              type="button"
              onClick={() => setKind(k)}
              className={`flex-1 rounded-pill px-3 py-1.5 text-[13px] capitalize ${
                kind === k ? "bg-accent-grad text-white" : "text-text-secondary"
              }`}
            >
              {k}
            </button>
          ))}
        </div>
      ) : null}

      <Field label="Name">
        <TextInput value={name} onChange={(e) => setName(e.target.value)} autoFocus />
      </Field>
      <Field label="Color">
        <ColorPicker value={color} onChange={setColor} colors={PALETTE} />
      </Field>

      <Field label={isFolder ? "Parent folder" : "Folder"}>
        <Select value={folderId} onChange={(e) => setFolderId(e.target.value)}>
          <option value="">— None —</option>
          {folders
            .filter((f) => !task || f.id !== task.id)
            .map((f) => (
              <option key={f.id} value={f.id}>
                {f.name}
              </option>
            ))}
        </Select>
      </Field>

      {!isFolder ? (
        <>
          <div className="flex gap-3">
            <div className="flex-1">
              <Field label="Hourly rate">
                <TextInput
                  type="number"
                  value={rate}
                  min="0"
                  step="1"
                  onChange={(e) => setRate(e.target.value)}
                />
              </Field>
            </div>
            <div className="w-28">
              <Field label="Currency">
                <TextInput value={currency} onChange={(e) => setCurrency(e.target.value.toUpperCase())} />
              </Field>
            </div>
          </div>
          <div className="mb-3 flex flex-col gap-2">
            <Toggle checked={billable} onChange={setBillable} label="Billable" />
            {editing ? (
              <Toggle checked={archived} onChange={setArchived} label="Archived" />
            ) : null}
          </div>
        </>
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
            {editing ? "Save" : "Create"}
          </Btn>
        </div>
      </div>
    </Modal>
  );
}
