import type { TreeNode } from "./types";

export interface TaskTotal {
  ms: number;
  rev: number;
  folderName: string | null;
}

/** Keep only nodes passing `keepTask`, pruning empty folders. */
export function filterTree(
  nodes: TreeNode[],
  keepTask: (n: TreeNode) => boolean,
  keepFolder: (n: TreeNode, keptChildren: TreeNode[]) => boolean = (_f, c) => c.length > 0,
): TreeNode[] {
  const out: TreeNode[] = [];
  for (const n of nodes) {
    if (n.kind === "task") {
      if (keepTask(n)) out.push(n);
    } else {
      const kids = filterTree(n.children, keepTask, keepFolder);
      if (keepFolder(n, kids)) out.push({ ...n, children: kids });
    }
  }
  return out;
}

/** Flatten a task tree into a taskId -> totals map (with folder eyebrow). */
export function flattenTaskTotals(
  nodes: TreeNode[],
  parentName: string | null = null,
  out: Map<string, TaskTotal> = new Map(),
): Map<string, TaskTotal> {
  for (const n of nodes) {
    if (n.kind === "task") {
      out.set(n.id, { ms: n.totalMs, rev: n.totalRevenue, folderName: parentName });
    } else {
      flattenTaskTotals(n.children, n.name, out);
    }
  }
  return out;
}
