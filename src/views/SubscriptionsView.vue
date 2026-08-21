<script setup lang="ts">
import { computed, h, reactive, ref, watch } from "vue";
import {
  NButton, NDataTable, NForm, NFormItem, NInput, NModal, NPopconfirm, NSpace, NStep, NSteps,
  NSwitch, NTag, NTooltip, type DataTableColumns, useMessage,
} from "naive-ui";
import { Activity, Link2, Pencil, Plus, RefreshCw, Trash2 } from "lucide-vue-next";
import PageHeader from "@/components/PageHeader.vue";
import StatusLabel from "@/components/StatusLabel.vue";
import { api } from "@/services/api";
import { useAppStore } from "@/stores/app";
import type { AvailableProxyNode, ConnectionTestResult, Subscription, SubscriptionInput } from "@/types";

const store = useAppStore();
const message = useMessage();
const modalVisible = ref(false);
const saving = ref(false);
const testing = ref(false);
const testingSubscriptionIds = ref<Set<string>>(new Set());
const testResult = ref<ConnectionTestResult | null>(null);
const savedTestResult = ref<ConnectionTestResult | null>(null);
const savedTestName = ref("");
const testDetailsVisible = ref(false);
const form = reactive<SubscriptionInput>({ name: "", url: "", enabled: true });
const editing = computed(() => Boolean(form.id));

const availableNodeColumns: DataTableColumns<AvailableProxyNode> = [
  { title: "可用节点", key: "name", minWidth: 260, ellipsis: { tooltip: true } },
  { title: "协议", key: "type", width: 92, render: (row) => h(NTag, { size: "small", bordered: false }, { default: () => row.type }) },
  { title: "延时", key: "elapsedMs", width: 88, align: "right", render: (row) => h("span", { class: "mono" }, `${row.elapsedMs}ms`) },
];

watch(() => form.url, () => {
  if (!testing.value) testResult.value = null;
});

function iconButton(icon: typeof Activity, label: string, onClick: () => void, loading = false) {
  return h(NTooltip, null, {
    trigger: () => h(NButton, { size: "small", quaternary: true, loading, "aria-label": label, onClick }, { icon: () => h(icon, { size: 16 }) }),
    default: () => label,
  });
}

const columns: DataTableColumns<Subscription> = [
  { title: "状态", key: "lastStatus", width: 92, render: (row) => h(StatusLabel, { status: row.lastStatus, text: row.lastStatus === "never" ? "未刷新" : undefined }) },
  { title: "名称", key: "name", minWidth: 140, sorter: "default" },
  { title: "订阅地址", key: "urlMasked", minWidth: 220, ellipsis: { tooltip: false }, render: (row) => h("span", { class: "mono muted" }, row.urlMasked) },
  { title: "节点", key: "proxyCount", width: 72, align: "right", render: (row) => h("span", { class: "mono" }, row.proxyCount) },
  { title: "最近成功", key: "lastSuccessAt", width: 150, render: (row) => row.lastSuccessAt ? new Date(row.lastSuccessAt).toLocaleString() : "-" },
  { title: "耗时", key: "elapsedMs", width: 82, align: "right", render: (row) => h("span", { class: "mono" }, row.elapsedMs ? `${row.elapsedMs}ms` : "-") },
  { title: "启用", key: "enabled", width: 68, render: (row) => h(NSwitch, { size: "small", value: row.enabled, onUpdateValue: async (value: boolean) => { await api.saveSubscription({ id: row.id, name: row.name, url: "", enabled: value }); await store.reloadSubscriptions(); } }) },
  { title: "操作", key: "actions", width: 126, fixed: "right", render: (row) => h(NSpace, { size: 2, wrap: false }, { default: () => [
    iconButton(Activity, "测试连接", () => testSavedSubscription(row), testingSubscriptionIds.value.has(row.id)),
    iconButton(Pencil, "编辑订阅", () => openEdit(row)),
    h(NPopconfirm, { onPositiveClick: () => remove(row.id) }, { trigger: () => iconButton(Trash2, "删除订阅", () => undefined), default: () => "删除后不会影响已发布版本，确认删除？" }),
  ] }) },
];

function openCreate() {
  Object.assign(form, { id: undefined, name: "", url: "", enabled: true });
  testResult.value = null;
  modalVisible.value = true;
}
function openEdit(row: Subscription) {
  Object.assign(form, { id: row.id, name: row.name, url: "", enabled: row.enabled });
  testResult.value = null;
  modalVisible.value = true;
}
async function testUrl() {
  if (!form.name.trim()) { message.error("请输入订阅名称"); return; }
  if (!editing.value && !form.url.trim()) { message.error("请输入订阅地址"); return; }
  testing.value = true;
  testResult.value = null;
  try {
    testResult.value = await api.testSubscriptionUrl(form.url, form.id);
  }
  finally { testing.value = false; }
}
async function testSavedSubscription(row: Subscription) {
  if (testingSubscriptionIds.value.has(row.id)) return;
  testingSubscriptionIds.value = new Set(testingSubscriptionIds.value).add(row.id);
  try {
    const result = await api.testSubscription(row.id);
    await store.reloadSubscriptions();
    savedTestResult.value = result;
    savedTestName.value = row.name;
    testDetailsVisible.value = true;
    if (result.reachable) message.success(`${row.name}：${result.availableProxyCount} 个节点可用`);
    else message.error(`${row.name}：${result.error ?? "连接失败"}`);
  } finally {
    const next = new Set(testingSubscriptionIds.value);
    next.delete(row.id);
    testingSubscriptionIds.value = next;
  }
}
async function save() {
  if (!form.name.trim()) { message.error("请输入订阅名称"); return; }
  if (!editing.value && !form.url.trim()) { message.error("请输入订阅地址"); return; }
  const wasEditing = editing.value;
  saving.value = true;
  try {
    await api.saveSubscription({ ...form, testResult: testResult.value ?? undefined });
    await store.reloadSubscriptions();
    modalVisible.value = false;
    message.success(wasEditing ? "订阅已更新" : "订阅已添加");
  } finally { saving.value = false; }
}
async function remove(id: string) {
  await api.deleteSubscription(id);
  await store.reloadSubscriptions();
  message.success("订阅已删除");
}
async function refreshAll() {
  const result = await store.refreshAll();
  message.success(`刷新完成：${result.successful} 个成功，${result.failed} 个失败`);
}
</script>

<template>
  <main class="page">
    <page-header title="订阅源" description="管理订阅地址并在保存前验证网络与配置格式">
      <n-button :loading="store.refreshing" :disabled="store.subscriptions.length === 0" @click="refreshAll"><template #icon><refresh-cw :size="16" /></template>刷新全部</n-button>
      <n-button type="primary" @click="openCreate"><template #icon><plus :size="16" /></template>添加订阅</n-button>
    </page-header>
    <section class="surface subscription-table">
      <div class="toolbar"><n-tag :bordered="false">共 {{ store.subscriptions.length }} 个</n-tag><span class="muted">完整订阅地址和响应内容不会写入界面日志</span></div>
      <n-data-table :columns="columns" :data="store.subscriptions" :row-key="(row) => row.id" :max-height="'calc(100vh - 190px)'" :virtual-scroll="store.subscriptions.length > 50" size="small" />
      <div v-if="store.subscriptions.length === 0" class="empty-state"><link2 :size="32" /><span>还没有订阅源</span><n-button type="primary" @click="openCreate">添加第一个订阅</n-button></div>
    </section>

    <n-modal v-model:show="modalVisible" preset="card" :title="editing ? '编辑订阅' : '添加订阅'" style="width:min(680px, calc(100vw - 32px))" :mask-closable="false">
      <n-form label-placement="top" @submit.prevent="save">
        <n-form-item label="名称" required><n-input v-model:value="form.name" placeholder="例如：主订阅" /></n-form-item>
        <n-form-item :label="editing ? '新订阅 URL（留空保持原地址）' : '订阅 URL'" :required="!editing">
          <n-input v-model:value="form.url" type="password" show-password-on="click" placeholder="https://example.com/subscription/token" class="mono" />
          <template #feedback>地址只用于请求；日志、表格和错误信息始终脱敏。</template>
        </n-form-item>
        <n-form-item label="启用"><n-switch v-model:value="form.enabled" /></n-form-item>
        <div class="connection-test">
          <div class="connection-test__header"><strong>连接测试</strong><n-button size="small" :loading="testing" @click="testUrl"><template #icon><activity :size="15" /></template>{{ testResult ? '重新测试' : '测试连接' }}</n-button></div>
          <n-steps size="small" :current="testResult?.reachable ? 5 : (testResult ? ['url','network','http','yaml','nodes'].indexOf(testResult.stage) + 1 : 0)" :status="testResult && !testResult.reachable ? 'error' : 'process'">
            <n-step title="URL" /><n-step title="网络" /><n-step title="HTTP" /><n-step title="YAML" /><n-step title="节点" />
          </n-steps>
          <div class="connection-test__result" aria-live="polite">
            <template v-if="testing">正在通过 Mihomo 发起真实代理请求，请稍候…</template>
            <template v-else-if="testResult">
              <status-label :status="testResult.reachable ? 'success' : 'error'" :text="testResult.reachable ? `${testResult.availableProxyCount} / ${testResult.proxyCount ?? 0} 个节点可用，耗时 ${testResult.elapsedMs}ms` : testResult.error ?? '连接失败'" />
              <n-data-table v-if="testResult.availableNodes.length" class="available-nodes" :columns="availableNodeColumns" :data="testResult.availableNodes" :row-key="(row: AvailableProxyNode) => row.index" :max-height="180" :virtual-scroll="testResult.availableNodes.length > 50" size="small" />
              <div v-if="testResult.warnings.length" class="test-warnings"><span v-for="warning in testResult.warnings" :key="warning">{{ warning }}</span></div>
            </template>
            <template v-else>测试成功要求至少一个节点完成真实代理请求。</template>
          </div>
        </div>
        <div class="modal-actions"><n-button @click="modalVisible = false">取消</n-button><n-button type="primary" attr-type="submit" :loading="saving">保存订阅</n-button></div>
      </n-form>
    </n-modal>

    <n-modal v-model:show="testDetailsVisible" preset="card" :title="`${savedTestName} · 节点测试`" style="width:min(680px, calc(100vw - 32px))">
      <template v-if="savedTestResult">
        <status-label :status="savedTestResult.reachable ? 'success' : 'error'" :text="savedTestResult.reachable ? `${savedTestResult.availableProxyCount} / ${savedTestResult.proxyCount ?? 0} 个节点可用，耗时 ${savedTestResult.elapsedMs}ms` : savedTestResult.error ?? '测试失败'" />
        <n-data-table v-if="savedTestResult.availableNodes.length" class="available-nodes" :columns="availableNodeColumns" :data="savedTestResult.availableNodes" :row-key="(row: AvailableProxyNode) => row.index" :max-height="420" :virtual-scroll="savedTestResult.availableNodes.length > 50" size="small" />
        <div v-if="savedTestResult.warnings.length" class="test-warnings"><span v-for="warning in savedTestResult.warnings" :key="warning">{{ warning }}</span></div>
      </template>
    </n-modal>
  </main>
</template>

<style scoped>
.subscription-table { overflow: hidden; }
.connection-test { min-height: 126px; padding: 12px; border: 1px solid var(--mc-border); border-radius: 4px; background: var(--mc-surface-muted); }
.connection-test__header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 12px; }
.connection-test__result { min-height: 24px; margin-top: 12px; color: var(--mc-text-secondary); }
.available-nodes { margin-top: 10px; }
.test-warnings { display: flex; flex-direction: column; gap: 4px; margin-top: 10px; color: var(--mc-warning); font-size: 12px; }
.modal-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 18px; }
</style>
