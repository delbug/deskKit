import type { YuqueProgressState } from '@/utils/appStorage';
import type { DocProgressItem, DocTreeNode } from '@/utils/yuque-doc-tree';

export type YuqueExportOrder = 'top-down' | 'bottom-up' | 'custom';

export interface YuqueCatalogSnapshot {
  version: 1;
  exportedAt: string;
  url: string;
  authMode?: 'share' | 'token';
  bookName: string;
  saveDir: string;
  bookDir?: string;
  exportOrder: YuqueExportOrder;
  /** null 表示全部文档（非自定义模式） */
  selectedSlugs: string[] | null;
  catalog: { slug: string; title: string; dirPath: string }[];
  docs: DocProgressItem[];
  tree: DocTreeNode[];
  progress?: YuqueProgressState | null;
}

export function buildYuqueSnapshot(input: {
  url: string;
  authMode?: 'share' | 'token';
  bookName: string;
  saveDir: string;
  bookDir?: string;
  exportOrder: YuqueExportOrder;
  selectedSlugs: string[] | null;
  catalog: { slug: string; title: string; dirPath: string }[];
  docs: DocProgressItem[];
  tree: DocTreeNode[];
  progress?: YuqueProgressState | null;
}): YuqueCatalogSnapshot {
  return {
    version: 1,
    exportedAt: new Date().toISOString(),
    url: input.url.trim(),
    authMode: input.authMode,
    bookName: input.bookName,
    saveDir: input.saveDir.trim(),
    bookDir: input.bookDir,
    exportOrder: input.exportOrder,
    selectedSlugs: input.exportOrder === 'custom' ? input.selectedSlugs : null,
    catalog: input.catalog,
    docs: input.docs,
    tree: input.tree,
    progress: input.progress ?? null,
  };
}

export function parseYuqueSnapshot(raw: string): YuqueCatalogSnapshot {
  const data = JSON.parse(raw) as YuqueCatalogSnapshot;
  if (data.version !== 1) {
    throw new Error('不支持的快照版本，请使用 DeskKit 导出的 JSON');
  }
  if (!data.url?.trim()) throw new Error('快照缺少语雀链接');
  if (!Array.isArray(data.catalog) || !data.catalog.length) {
    throw new Error('快照缺少知识库目录');
  }
  return data;
}

export function snapshotToProgress(snapshot: YuqueCatalogSnapshot): YuqueProgressState {
  const p = snapshot.progress;
  return {
    version: 1,
    url: snapshot.url,
    authMode: snapshot.authMode,
    bookName: snapshot.bookName,
    bookDir: snapshot.bookDir || p?.bookDir,
    saveDir: snapshot.saveDir,
    total: snapshot.catalog.length,
    completedSlugs: p?.completedSlugs || snapshot.docs.filter((d) => d.status === 'done').map((d) => d.slug),
    failed: p?.failed || snapshot.docs
      .filter((d) => d.status === 'failed')
      .map((d) => ({ slug: d.slug, title: d.title, message: d.failMessage || '失败' })),
    docManifest: snapshot.catalog.map((d) => ({
      slug: d.slug,
      title: d.title,
      dirPath: d.dirPath,
    })),
    exportOrder: snapshot.exportOrder,
    selectedSlugs: snapshot.selectedSlugs || undefined,
    status: p?.status || 'in_progress',
    startedAt: p?.startedAt,
    updatedAt: snapshot.exportedAt,
  };
}
