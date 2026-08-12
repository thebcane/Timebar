import { useMemo, useState } from "react";
import {
  ChevronRight,
  ChevronDown,
  Folder,
  Play,
  Pause,
  Pencil,
  Archive,
  ArchiveRestore,
  Trash2,
} from "lucide-react";
import { TaskRow } from "./TaskRow";
import { Eyebrow } from "./primitives";
import { ContextMenu, type MenuItem } from "./ContextMenu";
import { useStore } from "../store/store";
import { formatDuration } from "../lib/time";
import type { TreeNode } from "../lib/types";

type Kind = "task" | "folder";
type DropPos = "before" | "after" | "into";
interface DragItem {
  kind: Kind;
  id: string;
  parentId: string | null;
}

/**
 * Task tree with position-aware drag-and-drop: drop a row on the top/bottom of
 * another row to reorder within its parent, or on the middle of a folder to
 * move into it. Works for both tasks and folders (2-level nesting max).
 */
export function TaskTree({
  nodes,
  activeTaskId,
  loadedAt,
  onToggle,
  onEditTask,
  onArchive,
  onDelete,
  onMoveTask,
  onMoveFolder,
  onReorder,
}: {
  nodes: TreeNode[];
  activeTaskId?: string | null;
  loadedAt: number;
  onToggle: (taskId: string) => void;
  onEditTask: (taskId: string) => void;
  onArchive: (node: TreeNode) => void;
  onDelete: (node: TreeNode) => void;
  onMoveTask: (taskId: string, folderId: string | null) => Promise<void> | void;
  onMoveFolder: (folderId: string, parentId: string | null) => Promise<void> | void;
  onReorder: (entity: Kind, orderedIds: string[]) => Promise<void> | void;
}) {
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [menu, setMenu] = useState<{ x: number; y: number; node: TreeNode } | null>(null);
  const [drag, setDrag] = useState<DragItem | null>(null);
  const [over, setOver] = useState<{ id: string; pos: DropPos } | null>(null);
  const nowTick = useStore((s) => s.nowTick);

  // --- lookups derived from the tree ---------------------------------------
  const { byId, parentOf } = useMemo(() => {
    const byId = new Map<string, TreeNode>();
    const parentOf = new Map<string, string | null>();
    const walk = (list: TreeNode[], parent: string | null) => {
      for (const n of list) {
        byId.set(n.id, n);
        parentOf.set(n.id, parent);
        if (n.kind === "folder") walk(n.children, n.id);
      }
    };
    walk(nodes, null);
    return { byId, parentOf };
  }, [nodes]);

  const childIdsOfKind = (parentId: string | null, kind: Kind): string[] => {
    const list = parentId === null ? nodes : byId.get(parentId)?.children ?? [];
    return list.filter((n) => n.kind === kind).map((n) => n.id);
  };
  const hasSubfolders = (id: string) => (byId.get(id)?.children ?? []).some((c) => c.kind === "folder");
  const isSelfOrDescendant = (folderId: string, candidateId: string | null): boolean => {
    if (candidateId === null) return false;
    if (candidateId === folderId) return true;
    const node = byId.get(folderId);
    if (!node) return false;
    const stack = [...node.children];
    while (stack.length) {
      const c = stack.pop()!;
      if (c.id === candidateId) return true;
      if (c.kind === "folder") stack.push(...c.children);
    }
    return false;
  };

  const toggleCollapse = (id: string) =>
    setCollapsed((prev) => {
      const next = new Set(prev);
      next.has(id) ? next.delete(id) : next.add(id);
      return next;
    });

  // --- drop resolution -----------------------------------------------------
  const reorderAdjacent = async (D: DragItem, target: TreeNode, pos: DropPos, parent: string | null) => {
    if (D.kind === "task") {
      if (D.parentId !== parent) await onMoveTask(D.id, parent);
    } else if (D.parentId !== parent) {
      await onMoveFolder(D.id, parent);
    }
    const base = childIdsOfKind(parent, D.kind).filter((id) => id !== D.id);
    let idx = base.indexOf(target.id);
    if (idx < 0) idx = base.length;
    else if (pos === "after") idx += 1;
    base.splice(idx, 0, D.id);
    await onReorder(D.kind, base);
  };

  const resolveDrop = async (target: TreeNode) => {
    const D = drag;
    const pos = over?.pos ?? "after";
    if (!D || D.id === target.id) return;
    const targetParent = parentOf.get(target.id) ?? null;

    // Move INTO a folder.
    if (pos === "into" && target.kind === "folder") {
      if (D.kind === "task") {
        await onMoveTask(D.id, target.id);
      } else {
        // Nest a folder: not into itself/descendant, target must be top-level
        // (2-level max), and the dragged folder must have no subfolders.
        if (isSelfOrDescendant(D.id, target.id)) return;
        if (targetParent !== null || hasSubfolders(D.id)) return;
        await onMoveFolder(D.id, target.id);
      }
      return;
    }

    // Reorder relative to a sibling (before/after).
    const parent = targetParent;
    if (D.kind === "folder") {
      if (isSelfOrDescendant(D.id, parent)) return;
      if (parent !== null && hasSubfolders(D.id)) return; // would exceed 2 levels
      await reorderAdjacent(D, target, pos, parent);
    } else {
      await reorderAdjacent(D, target, pos, parent);
    }
  };

  const dndProps = (node: TreeNode) => ({
    draggable: true,
    onDragStart: (e: React.DragEvent) => {
      e.stopPropagation();
      // WebKit (Tauri's webview) needs drag data set or drops won't fire.
      e.dataTransfer.setData("text/plain", node.id);
      e.dataTransfer.effectAllowed = "move";
      setDrag({ kind: node.kind, id: node.id, parentId: parentOf.get(node.id) ?? null });
    },
    onDragEnd: () => {
      setDrag(null);
      setOver(null);
    },
    onDragOver: (e: React.DragEvent) => {
      if (!drag) return;
      e.preventDefault();
      e.stopPropagation();
      e.dataTransfer.dropEffect = "move";
      const rect = e.currentTarget.getBoundingClientRect();
      const y = e.clientY - rect.top;
      const h = rect.height;
      let pos: DropPos;
      if (node.kind === "folder") pos = y < h * 0.28 ? "before" : y > h * 0.72 ? "after" : "into";
      else pos = y < h * 0.5 ? "before" : "after";
      setOver({ id: node.id, pos });
    },
    onDrop: (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      void resolveDrop(node);
      setDrag(null);
      setOver(null);
    },
  });

  const indicator = (node: TreeNode) => {
    if (!drag || over?.id !== node.id) return null;
    if (over.pos === "into") return null; // handled via ring on the row
    return (
      <div
        className="pointer-events-none absolute left-2 right-2 h-0.5 rounded bg-accent"
        style={over.pos === "before" ? { top: -1 } : { bottom: -1 }}
      />
    );
  };

  // --- context menu items --------------------------------------------------
  const menuItems = (node: TreeNode): MenuItem[] => {
    if (node.kind === "task") {
      const running = activeTaskId === node.id;
      return [
        {
          label: running ? "Stop" : "Start",
          icon: running ? <Pause size={14} /> : <Play size={14} />,
          onClick: () => onToggle(node.id),
        },
        { label: "Edit…", icon: <Pencil size={14} />, onClick: () => onEditTask(node.id) },
        {
          label: node.archived ? "Unarchive" : "Archive",
          icon: node.archived ? <ArchiveRestore size={14} /> : <Archive size={14} />,
          onClick: () => onArchive(node),
        },
        { label: "Delete", icon: <Trash2 size={14} />, danger: true, onClick: () => onDelete(node) },
      ];
    }
    return [
      {
        label: node.archived ? "Unarchive" : "Archive",
        icon: node.archived ? <ArchiveRestore size={14} /> : <Archive size={14} />,
        onClick: () => onArchive(node),
      },
      { label: "Delete", icon: <Trash2 size={14} />, danger: true, onClick: () => onDelete(node) },
    ];
  };

  const openMenu = (e: React.MouseEvent, node: TreeNode) => {
    e.preventDefault();
    e.stopPropagation();
    setMenu({ x: e.clientX, y: e.clientY, node });
  };

  // --- rendering -----------------------------------------------------------
  const renderNodes = (list: TreeNode[], depth: number) =>
    list.map((node) => {
      const intoTarget = drag && over?.id === node.id && over.pos === "into";
      if (node.kind === "folder") {
        const isCollapsed = collapsed.has(node.id);
        return (
          <div key={node.id} className="relative">
            <div
              {...dndProps(node)}
              onClick={() => toggleCollapse(node.id)}
              onContextMenu={(e) => openMenu(e, node)}
              className={`relative flex cursor-pointer items-center gap-2 rounded-row px-2.5 py-2 hover:bg-panel-raised ${
                intoTarget ? "ring-1 ring-accent" : ""
              }`}
              style={{ paddingLeft: 10 + depth * 18 }}
            >
              {indicator(node)}
              {isCollapsed ? (
                <ChevronRight size={15} className="text-text-muted" />
              ) : (
                <ChevronDown size={15} className="text-text-muted" />
              )}
              <Folder size={16} style={{ color: node.color }} fill="currentColor" fillOpacity={0.25} />
              <span className="flex-1 truncate text-[14px] font-medium text-text-primary">{node.name}</span>
              <span className="tabular text-[13px] font-semibold text-text-secondary">
                {formatDuration(node.totalMs)}
              </span>
            </div>
            {!isCollapsed ? renderNodes(node.children, depth + 1) : null}
          </div>
        );
      }

      return (
        <div key={node.id} className="relative" {...dndProps(node)} onContextMenu={(e) => openMenu(e, node)}>
          {indicator(node)}
          <TaskRow
            name={node.name}
            color={node.color}
            totalMs={
              activeTaskId === node.id ? node.totalMs + Math.max(0, nowTick - loadedAt) : node.totalMs
            }
            revenue={node.totalRevenue}
            showRevenue={node.billable}
            active={activeTaskId === node.id}
            indent={depth}
            onToggle={(e) => {
              e.stopPropagation();
              onToggle(node.id);
            }}
            onClick={() => onEditTask(node.id)}
          />
        </div>
      );
    });

  return (
    <>
      <div
        onDragOver={(e) => drag && e.preventDefault()}
        onDrop={() => {
          // Drop on empty area → move to top level.
          if (drag && drag.parentId !== null) {
            if (drag.kind === "task") void onMoveTask(drag.id, null);
            else void onMoveFolder(drag.id, null);
          }
          setDrag(null);
          setOver(null);
        }}
        className="min-h-full pb-8"
      >
        {nodes.length === 0 ? (
          <Eyebrow className="px-2 py-3">No tasks yet. Add one with +.</Eyebrow>
        ) : (
          renderNodes(nodes, 0)
        )}
      </div>
      {menu ? (
        <ContextMenu x={menu.x} y={menu.y} items={menuItems(menu.node)} onClose={() => setMenu(null)} />
      ) : null}
    </>
  );
}
