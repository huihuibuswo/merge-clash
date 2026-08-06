<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  NButton, NButtonGroup, NCheckbox, NEmpty, NInput, NInputNumber, NList, NListItem, NModal, NPopconfirm, NRadioButton, NRadioGroup, NSelect, NSwitch, NTag, NTooltip, useMessage,
} from "naive-ui";
import { ArrowDown, ArrowUp, History, Plus, RotateCcw, Save as SaveIcon, Search as SearchIcon, Trash2 } from "lucide-vue-next";
import PageHeader from "@/components/PageHeader.vue";
import { api } from "@/services/api";
import { useAppStore } from "@/stores/app";
import type { DraftHistory, ProxyGroup } from "@/types";

const store = useAppStore();
const message = useMessage();
const search = ref("");
const sourceFilter = ref<string | null>(null);
const selectedNodeIds = ref<string[]>([]);
const selectedGroupIndex = ref(0);
const editMode = ref<"simple" | "advanced">("simple");
const saving = ref(false);
const groups = ref<ProxyGroup[]>([]);
const historyVisible = ref(false);
const historyLoading = ref(false);
const restoringHistoryId = ref<number | null>(null);
const historyItems = ref<DraftHistory[]>([]);

function cloneGroups(value: ProxyGroup[]) {
  return value.map((group) => ({ ...group, members: [...group.members] }));
}

watch(() => store.draft?.revision, () => {
  groups.value = cloneGroups(store.draft?.groups ?? []);
  selectedGroupIndex.value = Math.min(selectedGroupIndex.value, Math.max(groups.value.length - 1, 0));
}, { immediate: true });

const selectedGroup = computed(() => groups.value[selectedGroupIndex.value]);
const isGroupTemplate = computed(() => store.currentTemplate?.outputFormat === "mihomo-yaml");
const dynamicProviderMode = computed(() => store.draft?.mergeMode === "proxy-providers");
const sourceOptions = computed(() => Array.from(new Set(store.draft?.proxies.map((item) => item.sourceName) ?? [])).map((value) => ({ label: value, value })));
const filteredNodes = computed(() => (store.draft?.proxies ?? []).filter((item) => {
  const matchesText = !search.value || item.name.toLowerCase().includes(search.value.toLowerCase());
  return matchesText && (!sourceFilter.value || item.sourceName === sourceFilter.value);
}));
const groupTypeOptions = [
  { label: "手动选择", value: "select" }, { label: "自动测速", value: "url-test" },
  { label: "故障转移", value: "fallback" }, { label: "负载均衡", value: "load-balance" },
];
const countryOptions = ["美国", "日本", "新加坡", "台湾", "韩国", "加拿大", "英国", "澳大利亚"].map((value) => ({ label: value, value }));
const selectedCountries = computed({
  get: () => countryOptions.filter((item) => selectedGroup.value?.filter?.includes(item.value)).map((item) => item.value),
  set: (values: string[]) => { if (selectedGroup.value) selectedGroup.value.filter = values.length ? `(?i)(${values.join("|")})` : ""; },
});

function addSelectedNodes() {
  if (!selectedGroup.value) return;
  if (dynamicProviderMode.value) { message.warning("动态 Provider 模式请使用筛选规则选择节点"); return; }
  const names = (store.draft?.proxies ?? []).filter((item) => selectedNodeIds.value.includes(item.id)).map((item) => item.name);
  selectedGroup.value.members = Array.from(new Set([...selectedGroup.value.members, ...names]));
  selectedNodeIds.value = [];
}
function removeMember(index: number) { selectedGroup.value?.members.splice(index, 1); }
function moveMember(index: number, direction: -1 | 1) {
  if (!selectedGroup.value) return;
  const target = index + direction;
  if (target < 0 || target >= selectedGroup.value.members.length) return;
  const [item] = selectedGroup.value.members.splice(index, 1);
  selectedGroup.value.members.splice(target, 0, item);
}
function addGroup() {
  groups.value.push({ name: `新代理组 ${groups.value.length + 1}`, groupType: "select", members: ["DIRECT"] });
  selectedGroupIndex.value = groups.value.length - 1;
}
function deleteGroup() {
  if (!selectedGroup.value || selectedGroup.value.name === "节点选择") { message.warning("主选择组不能删除"); return; }
  groups.value.splice(selectedGroupIndex.value, 1);
  selectedGroupIndex.value = Math.max(0, selectedGroupIndex.value - 1);
}
async function save() {
  if (!store.draft) return;
  saving.value = true;
  try {
    store.draft = await api.saveGroups(store.draft.revision, groups.value);
    if (historyVisible.value) historyItems.value = await api.listDraftHistory();
    message.success("代理组已保存");
  } finally { saving.value = false; }
}
function reset() { groups.value = cloneGroups(store.draft?.groups ?? []); message.info("已恢复到最近保存状态"); }
async function openHistory() {
  historyVisible.value = true;
  historyLoading.value = true;
  try { historyItems.value = await api.listDraftHistory(); }
  finally { historyLoading.value = false; }
}
async function restoreHistory(id: number) {
  restoringHistoryId.value = id;
  try {
    store.draft = await api.restoreDraftHistory(id);
    historyItems.value = await api.listDraftHistory();
    historyVisible.value = false;
    message.success("已恢复历史版本");
  } finally { restoringHistoryId.value = null; }
}
</script>

<template>
  <main class="page groups-page">
    <page-header title="节点与分组" description="通过选择和筛选组织最终代理组">
      <n-button @click="openHistory"><template #icon><history :size="16" /></template>历史记录</n-button>
      <n-button v-if="isGroupTemplate" @click="reset"><template #icon><rotate-ccw :size="16" /></template>恢复</n-button>
      <n-button v-if="isGroupTemplate" type="primary" :loading="saving" :disabled="!store.draft" @click="save"><template #icon><save-icon :size="16" /></template>保存分组</n-button>
    </page-header>
    <n-empty v-if="!isGroupTemplate" class="surface groups-empty" description="当前通用 URI 模板不使用代理组；节点会按协议转换后直接输出。" />
    <n-empty v-else-if="!store.draft || store.draft.proxies.length === 0" class="surface groups-empty" description="刷新订阅后才能编辑节点分组">
      <template #extra><n-button type="primary" :loading="store.refreshing" @click="store.refreshAll">刷新订阅</n-button></template>
    </n-empty>
    <div v-else class="group-workspace surface">
      <section class="node-library">
        <div class="workspace-heading"><strong>节点库</strong><n-tag size="small" :bordered="false">{{ filteredNodes.length }}</n-tag></div>
        <div class="workspace-tools">
          <n-input v-model:value="search" size="small" clearable placeholder="搜索节点"><template #prefix><search-icon :size="14" /></template></n-input>
          <n-select v-model:value="sourceFilter" size="small" clearable placeholder="全部来源" :options="sourceOptions" />
        </div>
        <div class="node-list">
          <label v-for="node in filteredNodes" :key="node.id" class="node-row">
            <n-checkbox :checked="selectedNodeIds.includes(node.id)" :disabled="dynamicProviderMode" @update:checked="(checked) => checked ? selectedNodeIds.push(node.id) : selectedNodeIds.splice(selectedNodeIds.indexOf(node.id), 1)" />
            <span class="node-row__name truncate">{{ node.name }}</span>
            <n-tag size="tiny" :bordered="false">{{ node.type }}</n-tag>
          </label>
        </div>
        <div class="workspace-footer"><n-button block size="small" :disabled="dynamicProviderMode || selectedNodeIds.length === 0" @click="addSelectedNodes">加入当前组（{{ selectedNodeIds.length }}）</n-button></div>
      </section>

      <section class="group-list-panel">
        <div class="workspace-heading"><strong>代理组</strong><n-button size="tiny" quaternary aria-label="新建代理组" @click="addGroup"><template #icon><plus :size="15" /></template></n-button></div>
        <div class="group-list">
          <button v-for="(group, index) in groups" :key="`${group.name}-${index}`" class="group-row" :class="{ 'group-row--active': index === selectedGroupIndex }" @click="selectedGroupIndex = index">
            <span class="truncate">{{ group.name }}</span><n-tag size="tiny" :bordered="false">{{ group.members.length }}</n-tag>
          </button>
        </div>
      </section>

      <section v-if="selectedGroup" class="group-editor">
        <div class="workspace-heading"><strong>组编辑器</strong><n-tooltip><template #trigger><n-button size="tiny" quaternary type="error" aria-label="删除当前代理组" @click="deleteGroup"><template #icon><trash2 :size="15" /></template></n-button></template>删除当前代理组</n-tooltip></div>
        <div class="group-form">
          <div class="form-grid">
            <label><span>名称</span><n-input v-model:value="selectedGroup.name" size="small" /></label>
            <label><span>类型</span><n-select v-model:value="selectedGroup.groupType" size="small" :options="groupTypeOptions" /></label>
          </div>
          <template v-if="selectedGroup.groupType !== 'select'">
            <n-radio-group v-model:value="editMode" size="small">
              <n-radio-button value="simple">简单筛选</n-radio-button><n-radio-button value="advanced">高级正则</n-radio-button>
            </n-radio-group>
            <div v-if="editMode === 'simple'" class="filter-fields">
              <label><span>地区标签</span><n-select v-model:value="selectedCountries" multiple size="small" :options="countryOptions" placeholder="选择要包含的地区" /></label>
              <label><span>排除预设</span><n-input size="small" value="通知、流量、套餐、到期、更新" readonly /></label>
            </div>
            <div v-else class="filter-fields">
              <label><span>filter</span><n-input v-model:value="selectedGroup.filter" size="small" class="mono" /></label>
              <label><span>exclude-filter</span><n-input v-model:value="selectedGroup.excludeFilter" size="small" class="mono" /></label>
            </div>
            <div class="form-grid form-grid--metrics">
              <label><span>测速地址</span><n-input v-model:value="selectedGroup.url" size="small" class="mono" /></label>
              <label><span>间隔（秒）</span><n-input-number v-model:value="selectedGroup.interval" size="small" :min="30" :step="30" /></label>
              <label><span>容差</span><n-input-number v-model:value="selectedGroup.tolerance" size="small" :min="0" /></label>
              <label class="switch-field"><span>Lazy</span><n-switch :value="Boolean(selectedGroup.lazy)" size="small" @update:value="(value) => selectedGroup!.lazy = value" /></label>
            </div>
          </template>
          <div class="member-heading"><strong>成员</strong><span>{{ selectedGroup.members.length }} 项</span></div>
          <div class="member-list">
            <div v-for="(member, index) in selectedGroup.members" :key="`${member}-${index}`" class="member-row">
              <span class="member-row__index mono">{{ index + 1 }}</span><span class="truncate">{{ member }}</span>
              <n-button-group size="tiny">
                <n-button :disabled="index === 0" aria-label="上移" @click="moveMember(index, -1)"><template #icon><arrow-up :size="13" /></template></n-button>
                <n-button :disabled="index === selectedGroup.members.length - 1" aria-label="下移" @click="moveMember(index, 1)"><template #icon><arrow-down :size="13" /></template></n-button>
                <n-button aria-label="移除" @click="removeMember(index)"><template #icon><trash2 :size="13" /></template></n-button>
              </n-button-group>
            </div>
          </div>
        </div>
      </section>
    </div>
    <n-modal v-model:show="historyVisible" preset="card" title="节点与分组历史" style="width:680px" :mask-closable="false">
      <n-empty v-if="!historyLoading && historyItems.length === 0" description="暂无历史记录" />
      <n-list v-else :show-divider="true">
        <n-list-item v-for="item in historyItems" :key="item.id">
          <div class="history-row">
            <div><strong>r{{ item.revision }} · {{ item.action }}</strong><span>{{ new Date(item.createdAt).toLocaleString() }} · {{ item.nodeCount }} 个节点 · {{ item.groupCount }} 个分组</span></div>
            <n-popconfirm @positive-click="restoreHistory(item.id)"><template #trigger><n-button size="small" :loading="restoringHistoryId === item.id">恢复</n-button></template>恢复后会覆盖当前节点与分组草稿，是否继续？</n-popconfirm>
          </div>
        </n-list-item>
      </n-list>
    </n-modal>
  </main>
</template>

<style scoped>
.groups-page { height: 100%; display: flex; flex-direction: column; }
.groups-empty { flex: 1; }
.group-workspace { flex: 1; min-height: 0; display: grid; grid-template-columns: 300px 240px minmax(420px, 1fr); overflow: hidden; }
.node-library, .group-list-panel, .group-editor { min-width: 0; display: flex; flex-direction: column; }
.node-library, .group-list-panel { border-right: 1px solid var(--mc-border); }
.workspace-heading { flex: 0 0 40px; padding: 0 10px; display: flex; align-items: center; justify-content: space-between; border-bottom: 1px solid var(--mc-border); }
.workspace-tools { display: grid; gap: 6px; padding: 8px; border-bottom: 1px solid var(--mc-border); }
.node-list, .group-list, .member-list { min-height: 0; overflow: auto; }
.node-list { flex: 1; }
.node-row { min-height: 36px; padding: 0 8px; display: grid; grid-template-columns: 24px minmax(0, 1fr) auto; align-items: center; gap: 6px; cursor: pointer; }
.node-row:hover, .group-row:hover, .member-row:hover { background: var(--mc-surface-muted); }
.node-row__name { font-size: 13px; }
.workspace-footer { padding: 8px; border-top: 1px solid var(--mc-border); }
.group-list { flex: 1; padding: 5px; }
.group-row { width: 100%; min-height: 36px; padding: 0 8px; display: flex; align-items: center; justify-content: space-between; gap: 8px; color: inherit; background: transparent; border: 1px solid transparent; border-radius: 4px; cursor: pointer; text-align: left; }
.group-row--active { color: var(--mc-primary); background: color-mix(in srgb, var(--mc-primary) 10%, transparent); border-color: color-mix(in srgb, var(--mc-primary) 40%, var(--mc-border)); }
.group-form { padding: 12px; min-height: 0; overflow: auto; }
.form-grid { display: grid; grid-template-columns: 1fr 180px; gap: 10px; margin-bottom: 12px; }
.form-grid--metrics { grid-template-columns: minmax(220px, 1fr) 110px 90px 70px; margin-top: 12px; }
label > span { display: block; margin-bottom: 5px; color: var(--mc-text-secondary); font-size: 12px; }
.filter-fields { display: grid; gap: 10px; margin-top: 12px; }
.switch-field { display: flex; flex-direction: column; align-items: flex-start; }
.member-heading { min-height: 38px; margin-top: 8px; display: flex; align-items: center; justify-content: space-between; border-bottom: 1px solid var(--mc-border); }
.member-heading span { color: var(--mc-text-secondary); font-size: 12px; }
.member-list { max-height: 310px; }
.member-row { min-height: 36px; padding: 0 4px; display: grid; grid-template-columns: 28px minmax(0, 1fr) auto; align-items: center; gap: 6px; border-bottom: 1px solid color-mix(in srgb, var(--mc-border) 70%, transparent); }
.member-row__index { color: var(--mc-text-secondary); text-align: right; }
.history-row { width: 100%; display: flex; align-items: center; justify-content: space-between; gap: 16px; }
.history-row > div { min-width: 0; display: flex; flex-direction: column; gap: 4px; }
.history-row span { color: var(--mc-text-secondary); font-size: 12px; }
@media (max-width: 1199px) { .group-workspace { grid-template-columns: 220px minmax(480px, 1fr); } .node-library { display: none; } }
</style>
