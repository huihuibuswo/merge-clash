import { invoke } from "@tauri-apps/api/core";
import type {
  ConnectionTestResult,
  Draft,
  DraftHistory,
  ProjectSettings,
  ProxyGroup,
  PublishStatus,
  PublishedVersion,
  RefreshResult,
  Subscription,
  SubscriptionInput,
  TemplateSummary,
} from "@/types";

const isTauri = () => "__TAURI_INTERNALS__" in window;
const STORAGE_KEY = "merge-clash-browser-state-v1";

interface BrowserState {
  settings: ProjectSettings;
  subscriptions: Array<Subscription & { url: string }>;
  draft: Draft;
  publish: PublishStatus;
  history: BrowserDraftHistory[];
  versions: BrowserPublishedVersion[];
}

interface BrowserDraftHistory extends DraftHistory {
  draft: Draft;
}

interface BrowserPublishedVersion extends PublishedVersion {
  yaml: string;
}

const templates: TemplateSummary[] = [
  {
    id: "clash-mihomo",
    version: 1,
    name: "Clash / Mihomo",
    description: "适用于 Clash Meta、Mihomo 及其兼容客户端的通用 YAML 配置。",
    core: "Clash / Mihomo",
    outputFormat: "mihomo-yaml",
    fileName: "merge-clash.yaml",
    supportedModes: ["proxy-providers", "embedded-proxies"],
    defaultMode: "proxy-providers",
    groups: ["节点选择", "发达地区自动", "美国自动"],
    externalDependencies: ["MetaCubeX 中国大陆域名规则集"],
  },
  {
    id: "v2rayn",
    version: 1,
    name: "v2RayN",
    description: "Base64 编码的通用分享链接订阅，支持 SS、VMess、VLESS 和 Trojan。",
    core: "v2RayN",
    outputFormat: "base64-uri-list",
    fileName: "v2rayn.txt",
    supportedModes: ["embedded-proxies"],
    defaultMode: "embedded-proxies",
    groups: [],
    externalDependencies: [],
  },
  {
    id: "trojan",
    version: 1,
    name: "Trojan",
    description: "仅包含 Trojan 节点的 Base64 通用 URI 订阅。",
    core: "Trojan URI",
    outputFormat: "base64-uri-list",
    fileName: "trojan.txt",
    supportedModes: ["embedded-proxies"],
    defaultMode: "embedded-proxies",
    groups: [],
    externalDependencies: [],
  },
  {
    id: "shadowrocket",
    version: 1,
    name: "Shadowrocket",
    description: "适用于 Shadowrocket 的 Base64 分享链接订阅，支持 SS、VMess、VLESS 和 Trojan。",
    core: "Shadowrocket",
    outputFormat: "base64-uri-list",
    fileName: "shadowrocket.txt",
    supportedModes: ["embedded-proxies"],
    defaultMode: "embedded-proxies",
    groups: [],
    externalDependencies: [],
  },
];

function baseGroups(templateId: string): ProxyGroup[] {
  if (templateId !== "clash-mihomo") return [];
  const autoName = "发达地区自动";
  const groups: ProxyGroup[] = [
    { name: "节点选择", groupType: "select", members: [autoName, "美国自动", "DIRECT"] },
    {
      name: autoName,
      groupType: "url-test",
      members: [],
      filter: "(?i)(美国|日本|新加坡|台湾|us|jp|sg|tw)",
      excludeFilter: "(?i)(剩余|流量|套餐|到期|重置|通知|更新)",
      url: "https://www.gstatic.com/generate_204",
      interval: 300,
      tolerance: 50,
      lazy: true,
    },
    {
      name: "美国自动",
      groupType: "url-test",
      members: [],
      filter: "(?i)(美国|美國|美西|美东|us|usa|united states)",
      excludeFilter: "(?i)(剩余|流量|套餐|到期|重置|通知|更新)",
      url: "https://www.gstatic.com/generate_204",
      interval: 300,
      tolerance: 50,
      lazy: true,
    },
  ];
  return groups;
}

function emptyDraft(settings: ProjectSettings): Draft {
  return {
    revision: 1,
    templateId: settings.templateId,
    templateVersion: settings.templateVersion,
    mergeMode: settings.mergeMode,
    proxies: [],
    groups: baseGroups(settings.templateId),
    yaml: settings.templateId === "clash-mihomo" ? "# 添加订阅并刷新后生成配置\n" : "",
    issues: [{ severity: "warning", code: "no-subscriptions", message: "尚未添加可用订阅" }],
    sourceFailures: [],
    updatedAt: Date.now(),
  };
}

function loadBrowserState(): BrowserState {
  const saved = localStorage.getItem(STORAGE_KEY);
  if (saved) {
    const state = JSON.parse(saved) as Partial<BrowserState>;
    state.history ??= [];
    state.versions ??= [];
    if (state.settings && ["clash-verge-rev", "flclash", "mihomo-generic"].includes(state.settings.templateId)) {
      state.settings.templateId = "clash-mihomo";
      state.settings.templateVersion = 1;
      state.settings.mergeMode = "proxy-providers";
      state.draft = emptyDraft(state.settings);
    }
    return state as BrowserState;
  }
  const settings: ProjectSettings = {
    templateId: "clash-mihomo",
    templateVersion: 1,
    mergeMode: "proxy-providers",
    theme: "system",
  };
  const state: BrowserState = {
    settings,
    subscriptions: [],
    draft: emptyDraft(settings),
    publish: { running: false, port: 17890, bindAddress: "0.0.0.0", lanAddresses: ["192.168.1.20"], proxyDetected: false },
    history: [],
    versions: [],
  };
  saveBrowserState(state);
  return state;
}

function cloneDraft(draft: Draft): Draft {
  return JSON.parse(JSON.stringify(draft)) as Draft;
}

function recordHistory(state: BrowserState, action: string) {
  const nextId = (state.history[0]?.id ?? 0) + 1;
  state.history.unshift({
    id: nextId,
    revision: state.draft.revision,
    action,
    nodeCount: state.draft.proxies.length,
    groupCount: state.draft.groups.length,
    createdAt: Date.now(),
    draft: cloneDraft(state.draft),
  });
  state.history = state.history.slice(0, 50);
}

function saveBrowserState(state: BrowserState) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
}

function maskUrl(value: string) {
  try {
    const url = new URL(value);
    return `${url.protocol}//${url.host}/...`;
  } catch {
    return "无效地址";
  }
}

function renderMockContent(state: BrowserState) {
  if (state.settings.templateId !== "clash-mihomo") {
    const allowed = state.settings.templateId === "trojan" ? new Set(["trojan"]) : new Set(["ss", "vmess", "vless", "trojan"]);
    const links = state.draft.proxies.filter((item) => allowed.has(item.type)).map((item) => `${item.type}://demo@example.com:443#${encodeURIComponent(item.name)}`);
    return btoa(links.join("\n"));
  }
  const providers = state.subscriptions
    .filter((item) => item.enabled)
    .map((item, index) => `  sub_${index + 1}:\n    type: http\n    url: \"${item.url}\"\n    path: ./providers/sub_${index + 1}.yaml\n    interval: 86400`)
    .join("\n");
  const proxyNames = state.draft.proxies.map((item) => `  - name: \"${item.name}\"\n    type: ${item.type}\n    server: 127.0.0.1\n    port: 443`).join("\n");
  const groupNames = new Set(state.draft.groups.map((group) => group.name));
  const builtins = new Set(["DIRECT", "REJECT", "REJECT-DROP", "PASS", "GLOBAL"]);
  const groupYaml = state.draft.groups.map((group) => {
    if (state.settings.mergeMode === "proxy-providers") {
      const providerRefs = state.subscriptions.filter((item) => item.enabled).map((_, index) => `      - sub_${index + 1}`).join("\n");
      const members = group.members.filter((item) => groupNames.has(item) || builtins.has(item)).map((item) => `      - \"${item}\"`).join("\n");
      return `  - name: \"${group.name}\"\n    type: ${group.groupType}\n    use:\n${providerRefs || "      []"}${members ? `\n    proxies:\n${members}` : ""}`;
    }
    const members = group.members.map((item) => `      - \"${item}\"`).join("\n");
    return `  - name: \"${group.name}\"\n    type: ${group.groupType}\n    proxies:\n${members || "      - DIRECT"}`;
  }).join("\n");
  return `mixed-port: 7890\nallow-lan: true\nmode: rule\n${state.settings.mergeMode === "proxy-providers" ? `proxy-providers:\n${providers || "  {}"}` : `proxies:\n${proxyNames || "  []"}`}\nproxy-groups:\n${groupYaml}\nrules:\n  - GEOIP,CN,DIRECT\n  - MATCH,节点选择\n`;
}

async function mockInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  await new Promise((resolve) => setTimeout(resolve, command.includes("test") ? 650 : 120));
  const state = loadBrowserState();
  switch (command) {
    case "list_templates": return templates as T;
    case "get_project_settings": return state.settings as T;
    case "select_project_template": {
      state.settings.templateId = String(args?.templateId);
      state.settings.templateVersion = Number(args?.templateVersion ?? 1);
      state.settings.mergeMode = args?.mergeMode as ProjectSettings["mergeMode"];
      state.draft = emptyDraft(state.settings);
      saveBrowserState(state);
      return state.settings as T;
    }
    case "save_theme": {
      state.settings.theme = args?.theme as ProjectSettings["theme"];
      saveBrowserState(state);
      return state.settings as T;
    }
    case "list_subscriptions": return state.subscriptions.map(({ url: _url, ...item }) => item) as T;
    case "save_subscription": {
      const input = args?.input as SubscriptionInput;
      const existing = input.id ? state.subscriptions.find((item) => item.id === input.id) : undefined;
      const item = existing ?? {
        id: crypto.randomUUID(),
        name: input.name,
        url: input.url,
        urlMasked: maskUrl(input.url),
        enabled: input.enabled,
        priority: state.subscriptions.length,
        lastStatus: "never" as const,
        proxyCount: 0,
      };
      Object.assign(item, { name: input.name, url: input.url || item.url, urlMasked: maskUrl(input.url || item.url), enabled: input.enabled });
      if (input.testResult) {
        const now = Date.now();
        item.lastStatus = input.testResult.reachable ? "success" : "error";
        item.lastError = input.testResult.error ?? null;
        item.lastFetchedAt = now;
        item.lastTestedAt = now;
        item.lastSuccessAt = input.testResult.reachable ? now : item.lastSuccessAt;
        item.proxyCount = input.testResult.proxyCount ?? 0;
        item.elapsedMs = input.testResult.elapsedMs;
      }
      if (!existing) state.subscriptions.push(item);
      saveBrowserState(state);
      const { url: _url, ...safe } = item;
      return safe as T;
    }
    case "delete_subscription": {
      state.subscriptions = state.subscriptions.filter((item) => item.id !== args?.id);
      saveBrowserState(state);
      return undefined as T;
    }
    case "test_subscription_url": {
      const saved = state.subscriptions.find((subscription) => subscription.id === args?.id);
      const url = String(args?.url || saved?.url || "");
      const valid = /^https?:\/\//i.test(url);
      return {
        reachable: valid,
        stage: valid ? "complete" : "url",
        httpStatus: valid ? 200 : null,
        elapsedMs: 642,
        responseBytes: valid ? 18420 : null,
        proxyCount: valid ? 42 : null,
        proxyTypes: valid ? ["ss", "vmess", "trojan"] : [],
        warnings: [],
        error: valid ? null : "仅支持 http 或 https 订阅地址",
      } as T;
    }
    case "test_subscription": {
      const item = state.subscriptions.find((subscription) => subscription.id === args?.id);
      if (!item) throw new Error("订阅不存在或已删除");
      const valid = /^https?:\/\//i.test(item.url);
      const now = Date.now();
      item.lastStatus = valid ? "success" : "error";
      item.lastError = valid ? null : "仅支持 http 或 https 订阅地址";
      item.lastFetchedAt = now;
      item.lastTestedAt = now;
      item.lastSuccessAt = valid ? now : item.lastSuccessAt;
      item.proxyCount = valid ? 42 : 0;
      item.elapsedMs = 642;
      saveBrowserState(state);
      return {
        reachable: valid,
        stage: valid ? "complete" : "url",
        httpStatus: valid ? 200 : null,
        elapsedMs: 642,
        responseBytes: valid ? 18420 : null,
        proxyCount: valid ? 42 : null,
        proxyTypes: valid ? ["ss", "vmess", "trojan"] : [],
        warnings: [],
        error: valid ? null : "仅支持 http 或 https 订阅地址",
      } as T;
    }
    case "refresh_subscriptions": {
      const enabled = state.subscriptions.filter((item) => item.enabled);
      enabled.forEach((item, index) => {
        item.lastStatus = "success";
        item.lastSuccessAt = Date.now();
        item.lastFetchedAt = Date.now();
        item.lastTestedAt = Date.now();
        item.proxyCount = 12 + index * 7;
        item.elapsedMs = 420 + index * 83;
      });
      state.draft.proxies = enabled.flatMap((item, sourceIndex) => Array.from({ length: Math.min(item.proxyCount, 24) }, (_, index) => ({
        id: `${item.id}-${index}`,
        name: `${["美国", "日本", "新加坡", "台湾"][index % 4]}-${String(index + 1).padStart(2, "0")}`,
        type: ["ss", "vmess", "trojan"][index % 3],
        sourceId: item.id,
        sourceName: item.name,
      })));
      state.draft.groups = baseGroups(state.settings.templateId).map((group) => ({
        ...group,
        members: group.groupType === "select" || state.settings.mergeMode === "proxy-providers"
          ? group.members
          : state.draft.proxies.map((item) => item.name),
      }));
      state.draft.yaml = renderMockContent(state);
      state.draft.issues = state.settings.mergeMode === "proxy-providers" ? [{ severity: "warning", code: "sensitive-provider-urls", message: "动态模式生成文件包含原始订阅地址" }] : [];
      if (state.settings.templateId === "trojan" && !state.draft.proxies.some((item) => item.type === "trojan")) state.draft.issues.push({ severity: "blocker", code: "no-compatible-proxies", message: "没有可用于 Trojan 订阅的节点" });
      state.draft.revision += 1;
      state.draft.updatedAt = Date.now();
      recordHistory(state, "刷新订阅");
      saveBrowserState(state);
      return { draft: state.draft, successful: enabled.length, failed: 0 } as T;
    }
    case "get_draft": return state.draft as T;
    case "save_proxy_groups": {
      state.draft.groups = args?.groups as ProxyGroup[];
      if (state.settings.mergeMode === "proxy-providers") {
        const groupNames = new Set(state.draft.groups.map((group) => group.name));
        const builtins = new Set(["DIRECT", "REJECT", "REJECT-DROP", "PASS", "GLOBAL"]);
        state.draft.groups = state.draft.groups.map((group) => ({
          ...group,
          members: group.members.filter((item) => item !== group.name && (groupNames.has(item) || builtins.has(item))),
        }));
      }
      state.draft.revision += 1;
      state.draft.yaml = renderMockContent(state);
      state.draft.updatedAt = Date.now();
      recordHistory(state, "保存分组");
      saveBrowserState(state);
      return state.draft as T;
    }
    case "save_draft_yaml": {
      if (state.draft.revision !== Number(args?.revision)) throw new Error("草稿已被其他操作更新，请刷新页面后重试");
      const yaml = String(args?.yaml ?? "");
      if (!yaml.trim()) throw new Error("配置内容不能为空");
      if (state.draft.templateId === "clash-mihomo" && !/^\s*[\w"'-]+\s*:/m.test(yaml)) throw new Error("YAML 顶层必须是对象");
      if (state.draft.templateId !== "clash-mihomo") { try { atob(yaml.trim()); } catch { throw new Error("订阅内容必须是有效的 Base64 文本"); } }
      state.draft.yaml = yaml;
      state.draft.revision += 1;
      state.draft.updatedAt = Date.now();
      recordHistory(state, "保存 YAML");
      saveBrowserState(state);
      return state.draft as T;
    }
    case "list_draft_history": return state.history.map(({ draft: _draft, ...item }) => item) as T;
    case "restore_draft_history": {
      const item = state.history.find((entry) => entry.id === Number(args?.id));
      if (!item) throw new Error("历史记录不存在或已清理");
      if (item.draft.templateId !== state.settings.templateId
        || item.draft.templateVersion !== state.settings.templateVersion
        || item.draft.mergeMode !== state.settings.mergeMode) {
        throw new Error("该历史记录属于其他模板或合并模式，无法恢复");
      }
      const publishedAt = state.draft.publishedAt;
      state.draft = cloneDraft(item.draft);
      state.draft.revision = Math.max(...state.history.map((entry) => entry.revision), state.draft.revision) + 1;
      state.draft.updatedAt = Date.now();
      state.draft.publishedAt = publishedAt;
      recordHistory(state, "恢复历史");
      saveBrowserState(state);
      return state.draft as T;
    }
    case "delete_draft_history": {
      const id = Number(args?.id);
      const item = state.history.find((entry) => entry.id === id);
      if (!item || item.revision === state.draft.revision) throw new Error("草稿历史不存在，或该记录属于当前草稿");
      state.history = state.history.filter((entry) => entry.id !== id);
      saveBrowserState(state);
      return state.history.map(({ draft: _draft, ...entry }) => entry) as T;
    }
    case "delete_other_draft_history": {
      state.history = state.history.filter((entry) => entry.revision === state.draft.revision);
      saveBrowserState(state);
      return state.history.map(({ draft: _draft, ...entry }) => entry) as T;
    }
    case "publish_draft": {
      state.draft.publishedAt = Date.now();
      state.publish.lastPublishedAt = Date.now();
      state.publish.versionNo = Math.max(0, ...state.versions.map((item) => item.versionNo)) + 1;
      state.publish.contentHash = crypto.randomUUID().replaceAll("-", "");
      state.versions.forEach((item) => { item.active = false; });
      state.versions.unshift({
        versionNo: state.publish.versionNo,
        templateId: state.draft.templateId,
        templateVersion: state.draft.templateVersion,
        mergeMode: state.draft.mergeMode,
        contentHash: state.publish.contentHash,
        createdAt: state.publish.lastPublishedAt,
        active: true,
        yaml: state.draft.yaml,
      });
      saveBrowserState(state);
      return state.publish as T;
    }
    case "list_published_versions": return state.versions.map(({ yaml: _yaml, ...item }) => item) as T;
    case "activate_published_version": {
      const version = state.versions.find((item) => item.versionNo === Number(args?.versionNo));
      if (!version) throw new Error("发布版本不存在或已删除");
      state.versions.forEach((item) => { item.active = item.versionNo === version.versionNo; });
      state.publish.versionNo = version.versionNo;
      state.publish.contentHash = version.contentHash;
      state.publish.lastPublishedAt = version.createdAt;
      state.publish.running = false;
      state.publish.subscriptionUrl = null;
      saveBrowserState(state);
      return state.publish as T;
    }
    case "delete_published_version": {
      const versionNo = Number(args?.versionNo);
      if (!state.versions.some((item) => item.versionNo === versionNo)) throw new Error("发布版本不存在或已删除");
      const deletingActive = state.publish.versionNo === versionNo;
      state.versions = state.versions.filter((item) => item.versionNo !== versionNo);
      if (deletingActive) {
        const fallback = state.versions[0];
        state.versions.forEach((item) => { item.active = item === fallback; });
        state.publish.versionNo = fallback?.versionNo ?? null;
        state.publish.contentHash = fallback?.contentHash ?? null;
        state.publish.lastPublishedAt = fallback?.createdAt ?? null;
        if (!fallback) {
          state.publish.running = false;
          state.publish.subscriptionUrl = null;
        }
      }
      saveBrowserState(state);
      return state.publish as T;
    }
    case "delete_other_published_versions": {
      const activeVersion = state.publish.versionNo;
      if (!activeVersion) throw new Error("当前没有发布版本");
      state.versions = state.versions.filter((item) => item.versionNo === activeVersion);
      saveBrowserState(state);
      return state.versions.map(({ yaml: _yaml, ...item }) => item) as T;
    }
    case "get_publish_status": return state.publish as T;
    case "start_publish_server": {
      state.publish.running = true;
      state.publish.subscriptionUrl = `http://${state.publish.lanAddresses[0]}:${state.publish.port}/subscription/demo-token/config`;
      saveBrowserState(state);
      return state.publish as T;
    }
    case "stop_publish_server": {
      state.publish.running = false;
      state.publish.subscriptionUrl = null;
      saveBrowserState(state);
      return state.publish as T;
    }
    case "save_publish_settings": {
      state.publish.port = Number(args?.port ?? 17890);
      saveBrowserState(state);
      return state.publish as T;
    }
    case "rotate_publish_token": {
      if (state.publish.running) state.publish.subscriptionUrl = `http://${state.publish.lanAddresses[0]}:${state.publish.port}/subscription/${crypto.randomUUID()}/config`;
      saveBrowserState(state);
      return state.publish as T;
    }
    default: throw new Error(`Browser mock does not implement ${command}`);
  }
}

function call<T>(command: string, args?: Record<string, unknown>) {
  return isTauri() ? invoke<T>(command, args) : mockInvoke<T>(command, args);
}

export const api = {
  listTemplates: () => call<TemplateSummary[]>("list_templates"),
  getSettings: () => call<ProjectSettings>("get_project_settings"),
  selectTemplate: (templateId: string, templateVersion: number, mergeMode: ProjectSettings["mergeMode"]) =>
    call<ProjectSettings>("select_project_template", { templateId, templateVersion, mergeMode }),
  saveTheme: (theme: ProjectSettings["theme"]) => call<ProjectSettings>("save_theme", { theme }),
  listSubscriptions: () => call<Subscription[]>("list_subscriptions"),
  saveSubscription: (input: SubscriptionInput) => call<Subscription>("save_subscription", { input }),
  deleteSubscription: (id: string) => call<void>("delete_subscription", { id }),
  testSubscriptionUrl: (url: string, id?: string) => call<ConnectionTestResult>("test_subscription_url", { url, id }),
  testSubscription: (id: string) => call<ConnectionTestResult>("test_subscription", { id }),
  refreshSubscriptions: () => call<RefreshResult>("refresh_subscriptions"),
  getDraft: () => call<Draft>("get_draft"),
  saveGroups: (revision: number, groups: ProxyGroup[]) => call<Draft>("save_proxy_groups", { revision, groups }),
  saveDraftYaml: (revision: number, yaml: string) => call<Draft>("save_draft_yaml", { revision, yaml }),
  listDraftHistory: () => call<DraftHistory[]>("list_draft_history"),
  restoreDraftHistory: (id: number) => call<Draft>("restore_draft_history", { id }),
  deleteDraftHistory: (id: number) => call<DraftHistory[]>("delete_draft_history", { id }),
  deleteOtherDraftHistory: () => call<DraftHistory[]>("delete_other_draft_history"),
  publishDraft: () => call<PublishStatus>("publish_draft"),
  listPublishedVersions: () => call<PublishedVersion[]>("list_published_versions"),
  activatePublishedVersion: (versionNo: number) => call<PublishStatus>("activate_published_version", { versionNo }),
  deletePublishedVersion: (versionNo: number) => call<PublishStatus>("delete_published_version", { versionNo }),
  deleteOtherPublishedVersions: () => call<PublishedVersion[]>("delete_other_published_versions"),
  getPublishStatus: () => call<PublishStatus>("get_publish_status"),
  startServer: () => call<PublishStatus>("start_publish_server"),
  stopServer: () => call<PublishStatus>("stop_publish_server"),
  savePublishSettings: (port: number) => call<PublishStatus>("save_publish_settings", { port }),
  rotateToken: () => call<PublishStatus>("rotate_publish_token"),
};
