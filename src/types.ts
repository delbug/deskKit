export type CompareMode = 'name' | 'md5';

export type DiffStatus =
  | 'identical'
  | 'missing'
  | 'content-diff'
  | 'relocated'
  | `only-${string}`;

export interface FolderItem {
  id: string;
  path: string;
  label: string;
  isPrimary: boolean;
}

export interface FavoriteItem {
  id: string;
  name: string;
  folders: FolderItem[];
  note?: string;
  createdAt: string;
  updatedAt?: string;
}

export interface AppSettings {
  compareMode: CompareMode;
  ignorePatterns: string[];
  defaultSyncStrategy: string;
}

export interface AppConfig {
  favorites: FavoriteItem[];
  recentPaths: string[];
  settings: AppSettings;
  lastSession: {
    folders: FolderItem[];
    compareMode: CompareMode;
  };
}

export interface DiffEntry {
  relativePath: string;
  status: DiffStatus;
  presence: Record<string, boolean>;
  sizes: Record<string, number | null>;
  md5s: Record<string, string | null>;
  mtimes: Record<string, number | null>;
  presentCount: number;
  primaryHas: boolean;
  primaryOnly: boolean;
  secondaryOnly: boolean;
  presentIds: string[];
  pathsByFolder?: Record<string, string>;
  md5?: string;
}

export interface CompareStats {
  total: number;
  identical: number;
  missing: number;
  contentDiff: number;
  relocated: number;
  onlyIn: Record<string, number>;
}

export interface CompareResult {
  mode: CompareMode;
  primaryId: string;
  folders: Record<string, {
    id: string;
    path: string;
    label: string;
    isPrimary: boolean;
    fileCount: number;
  }>;
  entries: DiffEntry[];
  stats: CompareStats;
}

export type SyncStrategy = 'primary-overwrite' | 'union' | 'selected';
export type FilterType = 'all' | 'diff' | 'identical' | 'missing' | 'content-diff' | 'relocated';

export interface SyncPreviewOperation {
  action: 'copy' | 'overwrite' | 'delete' | 'skip';
  relativePath: string;
  targetLabel?: string;
  detail?: string;
}

export interface SyncPreviewSummary {
  copy: number;
  overwrite: number;
  delete: number;
  skip: number;
  total: number;
}

export interface RenameRules {
  prefix?: string;
  suffix?: string;
  replaceFrom?: string;
  replaceTo?: string;
  replacements?: { from: string; to: string }[];
  removePatterns?: string[];
  includeExtension?: boolean;
  sequence?: {
    enabled: boolean;
    position: 'prefix' | 'suffix' | 'insert';
    insertIndex?: number;
    start: number;
    step: number;
    padWidth: number;
    separator?: string;
  };
  insert?: {
    enabled: boolean;
    index: number;
    content?: string;
    useSequence?: boolean;
  };
  deleteAt?: {
    enabled: boolean;
    /** 保留前几个字符，其后开始删除 */
    start: number;
    /** 删除几个字符 */
    count: number;
  };
}

export interface RenamePlanItem {
  relativePath: string;
  oldName: string;
  newName: string;
  newRelativePath: string;
  status: 'ready' | 'unchanged' | 'collision' | 'invalid';
  reason?: string;
  kind: 'file' | 'directory';
}

export interface DuplicateGroup {
  md5: string;
  size: number;
  count: number;
  files: { relativePath: string; absolutePath: string; size: number }[];
}
