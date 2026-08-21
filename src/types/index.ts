export type MergeMode = "proxy-providers" | "embedded-proxies";
export type Status = "never" | "success" | "error" | "running" | "warning";
export type ThemeMode = "system" | "light" | "dark";

export interface TemplateSummary {
  id: string;
  version: number;
  name: string;
  description: string;
  core: string;
  outputFormat: "mihomo-yaml" | "base64-uri-list";
  fileName: string;
  supportedModes: MergeMode[];
  defaultMode: MergeMode;
  groups: string[];
  externalDependencies: string[];
}

export interface ProjectSettings {
  templateId: string;
  templateVersion: number;
  mergeMode: MergeMode;
  theme: ThemeMode;
}

export interface Subscription {
  id: string;
  name: string;
  urlMasked: string;
  enabled: boolean;
  priority: number;
  lastStatus: Status;
  lastError?: string | null;
  lastFetchedAt?: number | null;
  lastSuccessAt?: number | null;
  lastTestedAt?: number | null;
  proxyCount: number;
  elapsedMs?: number | null;
}

export interface SubscriptionInput {
  id?: string;
  name: string;
  url: string;
  enabled: boolean;
  priority?: number;
  testResult?: ConnectionTestResult;
}

export type TestStage = "url" | "network" | "http" | "yaml" | "nodes" | "complete";

export interface AvailableProxyNode {
  index: number;
  name: string;
  type: string;
  elapsedMs: number;
}

export interface ConnectionTestResult {
  reachable: boolean;
  stage: TestStage;
  httpStatus?: number | null;
  elapsedMs: number;
  responseBytes?: number | null;
  proxyCount?: number | null;
  availableProxyCount: number;
  availableNodes: AvailableProxyNode[];
  proxyTypes: string[];
  warnings: string[];
  error?: string | null;
}

export interface ProxyNode {
  id: string;
  name: string;
  type: string;
  sourceId: string;
  sourceName: string;
}

export interface ProxyGroup {
  name: string;
  groupType: "select" | "url-test" | "fallback" | "load-balance";
  members: string[];
  filter?: string | null;
  excludeFilter?: string | null;
  url?: string | null;
  interval?: number | null;
  tolerance?: number | null;
  lazy?: boolean | null;
}

export interface ValidationIssue {
  severity: "blocker" | "warning" | "info";
  code: string;
  message: string;
  target?: string | null;
}

export interface Draft {
  revision: number;
  templateId: string;
  templateVersion: number;
  mergeMode: MergeMode;
  proxies: ProxyNode[];
  groups: ProxyGroup[];
  yaml: string;
  issues: ValidationIssue[];
  sourceFailures: string[];
  updatedAt: number;
  publishedAt?: number | null;
}

export interface RefreshResult {
  draft: Draft;
  successful: number;
  failed: number;
}

export interface PublishStatus {
  running: boolean;
  port: number;
  bindAddress: string;
  lanAddresses: string[];
  proxyDetected?: boolean;
  subscriptionUrl?: string | null;
  lastPublishedAt?: number | null;
  versionNo?: number | null;
  contentHash?: string | null;
  lastError?: string | null;
}

export interface DraftHistory {
  id: number;
  revision: number;
  action: string;
  nodeCount: number;
  groupCount: number;
  createdAt: number;
}

export interface PublishedVersion {
  versionNo: number;
  templateId: string;
  templateVersion: number;
  mergeMode: MergeMode;
  contentHash: string;
  createdAt: number;
  active: boolean;
}

export interface OverviewData {
  subscriptions: number;
  successfulSubscriptions: number;
  nodes: number;
  groups: number;
  blockers: number;
  warnings: number;
  draftDirty: boolean;
}
