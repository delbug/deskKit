export type DocStatus = 'pending' | 'done' | 'failed' | 'exporting';

export interface DocProgressItem {
  slug: string;
  title: string;
  dirPath: string;
  status: DocStatus;
  failMessage?: string;
}

export interface DocTreeNode {
  id: string;
  name: string;
  type: 'folder' | 'doc';
  children?: DocTreeNode[];
  doc?: DocProgressItem;
  doneCount?: number;
  totalCount?: number;
}

export interface ExportProgressDetail {
  found: boolean;
  bookName?: string;
  bookDir?: string;
  total?: number;
  completed?: number;
  remaining?: number;
  failedCount?: number;
  status?: string;
  currentSlug?: string | null;
  completedSlugs?: string[];
  failed?: { slug: string; title?: string; message: string }[];
  docs?: DocProgressItem[];
}

export function mergeDocProgress(
  catalog: { slug: string; title: string; dirPath: string }[],
  detail: ExportProgressDetail | null,
): DocProgressItem[] {
  if (!catalog.length && detail?.docs?.length) return detail.docs;
  if (!catalog.length) return [];

  const completed = new Set(detail?.completedSlugs || []);
  const failedMap = new Map((detail?.failed || []).map((f) => [f.slug, f.message]));
  const current = detail?.currentSlug;
  const statusFromDetail = new Map((detail?.docs || []).map((d) => [d.slug, d]));

  return catalog.map((d) => {
    const fromDetail = statusFromDetail.get(d.slug);
    if (fromDetail) {
      return {
        slug: d.slug,
        title: d.title,
        dirPath: d.dirPath,
        status: fromDetail.status,
        failMessage: fromDetail.failMessage,
      };
    }
    return {
      ...d,
      status: current === d.slug
        ? 'exporting'
        : completed.has(d.slug)
          ? 'done'
          : failedMap.has(d.slug)
            ? 'failed'
            : 'pending',
      failMessage: failedMap.get(d.slug),
    };
  });
}

export function buildDocTree(docs: DocProgressItem[]): DocTreeNode[] {
  const root: DocTreeNode = { id: '__root__', name: '', type: 'folder', children: [] };

  for (const doc of docs) {
    const parts = doc.dirPath === '(根目录)' ? [] : doc.dirPath.split('/').filter(Boolean);
    let node = root;
    const pathSoFar: string[] = [];
    for (const part of parts) {
      pathSoFar.push(part);
      let folder = node.children!.find((c) => c.type === 'folder' && c.name === part);
      if (!folder) {
        folder = {
          id: `folder:${pathSoFar.join('/')}`,
          name: part,
          type: 'folder',
          children: [],
        };
        node.children!.push(folder);
      }
      node = folder;
    }
    node.children!.push({
      id: `doc:${doc.slug}`,
      name: doc.title,
      type: 'doc',
      doc,
    });
  }

  annotateCounts(root);
  return root.children || [];
}

function annotateCounts(node: DocTreeNode): { done: number; total: number } {
  if (node.type === 'doc' && node.doc) {
    const done = node.doc.status === 'done' ? 1 : 0;
    node.doneCount = done;
    node.totalCount = 1;
    return { done, total: 1 };
  }

  let done = 0;
  let total = 0;
  for (const child of node.children || []) {
    const sub = annotateCounts(child);
    done += sub.done;
    total += sub.total;
  }
  node.doneCount = done;
  node.totalCount = total;
  return { done, total };
}

export function formatProgressBar(done: number, total: number, width = 24): string {
  if (total <= 0) return `[${'.'.repeat(width)}] 0/0`;
  const filled = Math.min(width, Math.max(0, Math.round((done / total) * width)));
  return `[${'|'.repeat(filled)}${'.'.repeat(width - filled)}] ${done}/${total}`;
}
