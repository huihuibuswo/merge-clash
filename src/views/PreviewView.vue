<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { NAlert, NButton, NDrawer, NDrawerContent, NEmpty, NInput, NList, NListItem, NPopconfirm, NTabPane, NTabs, NTag, useMessage } from "naive-ui";
import { FileCode2, History, RefreshCw, RotateCcw, Save, Trash2, Upload } from "lucide-vue-next";
import PageHeader from "@/components/PageHeader.vue";
import StatusLabel from "@/components/StatusLabel.vue";
import { api } from "@/services/api";
import { useAppStore } from "@/stores/app";
import type { DraftHistory } from "@/types";

const store = useAppStore();
const message = useMessage();
const publishing = ref(false);
const savingYaml = ref(false);
const yamlText = ref("");
const historyDrawerVisible = ref(false);
const historyItems = ref<DraftHistory[]>([]);
const historyLoading = ref(false);
const restoringHistoryId = ref<number | null>(null);
const deletingHistoryId = ref<number | null>(null);
const deletingOtherHistory = ref(false);
const blockers = computed(() => store.draft?.issues.filter((item) => item.severity === "blocker") ?? []);
const warnings = computed(() => store.draft?.issues.filter((item) => item.severity === "warning") ?? []);
const isYaml = computed(() => store.currentTemplate?.outputFormat === "mihomo-yaml");
const contentLabel = computed(() => isYaml.value ? "YAML" : "Base64 订阅内容");
const yamlDirty = computed(() => Boolean(store.draft && yamlText.value !== store.draft.yaml));
watch(() => store.draft?.revision, () => { yamlText.value = store.draft?.yaml ?? ""; }, { immediate: true });

async function saveYaml() {
  if (!store.draft) return;
  savingYaml.value = true;
  try {
    store.draft = await api.saveDraftYaml(store.draft.revision, yamlText.value);
    message.success(`${contentLabel.value}已保存`);
  } catch (error) {
    message.error(String(error));
  } finally { savingYaml.value = false; }
}
function resetYaml() { yamlText.value = store.draft?.yaml ?? ""; }
async function openHistoryDrawer() {
  historyDrawerVisible.value = true;
  historyLoading.value = true;
  try { historyItems.value = await api.listDraftHistory(); }
  finally { historyLoading.value = false; }
}
async function restoreHistory(id: number) {
  restoringHistoryId.value = id;
  try {
    store.draft = await api.restoreDraftHistory(id);
    historyItems.value = await api.listDraftHistory();
    message.success(`已切换到草稿版本 r${store.draft.revision}`);
  } finally { restoringHistoryId.value = null; }
}
async function deleteHistory(id: number) {
  deletingHistoryId.value = id;
  try {
    historyItems.value = await api.deleteDraftHistory(id);
    message.success("草稿历史已删除");
  } finally { deletingHistoryId.value = null; }
}
async function deleteOtherHistory() {
  deletingOtherHistory.value = true;
  try {
    historyItems.value = await api.deleteOtherDraftHistory();
    message.success("已删除当前草稿之外的全部历史版本");
  } finally { deletingOtherHistory.value = false; }
}
async function publish() {
  publishing.value = true;
  try {
    store.publishStatus = await api.publishDraft();
    if (store.draft) {
      store.draft = { ...store.draft, publishedAt: store.publishStatus.lastPublishedAt };
    }
    message.success(`已发布版本 v${store.publishStatus.versionNo}`);
  } finally { publishing.value = false; }
}
</script>

<template>
  <main class="page">
    <page-header title="配置预览" description="发布前检查问题、版本摘要和最终输出内容">
      <n-button :loading="store.refreshing" :disabled="store.subscriptions.length === 0" @click="store.refreshAll"><template #icon><refresh-cw :size="16" /></template>重新生成</n-button>
      <n-button type="primary" :loading="publishing" :disabled="!store.draft || blockers.length > 0" @click="publish"><template #icon><upload :size="16" /></template>发布当前草稿</n-button>
    </page-header>
    <n-empty v-if="!store.draft" class="surface empty-state" description="尚未生成草稿" />
    <template v-else>
      <section class="metric-strip preview-metrics">
        <div class="metric"><span>阻断</span><strong>{{ blockers.length }}</strong></div>
        <div class="metric"><span>警告</span><strong>{{ warnings.length }}</strong></div>
        <div class="metric"><span>节点</span><strong>{{ store.draft.proxies.length }}</strong></div>
        <div class="metric"><span>{{ isYaml ? '代理组' : '输出格式' }}</span><strong :style="isYaml ? undefined : { fontSize: '13px' }">{{ isYaml ? store.draft.groups.length : 'Base64 URI' }}</strong></div>
        <div class="metric"><span>模板</span><strong style="font-size:14px">{{ store.currentTemplate?.name }}</strong></div>
        <button class="metric metric-button" @click="openHistoryDrawer"><span>草稿版本</span><strong>r{{ store.draft.revision }}</strong><history :size="14" /></button>
      </section>
      <section class="surface section preview-panel">
        <n-tabs type="line" animated style="padding:0 12px">
          <n-tab-pane name="issues" tab="校验问题">
            <n-alert v-if="blockers.length" type="error" style="margin-bottom:10px">存在阻断问题，修复后才能发布。</n-alert>
            <n-list v-if="store.draft.issues.length">
              <n-list-item v-for="issue in store.draft.issues" :key="`${issue.code}-${issue.target}`">
                <div class="issue-row"><status-label :status="issue.severity === 'blocker' ? 'error' : issue.severity === 'warning' ? 'warning' : 'never'" :text="issue.severity === 'blocker' ? '阻断' : issue.severity === 'warning' ? '警告' : '信息'" /><div><strong>{{ issue.message }}</strong><span v-if="issue.target">{{ issue.target }}</span></div></div>
              </n-list-item>
            </n-list>
            <n-empty v-else description="校验通过，没有发现问题" />
          </n-tab-pane>
          <n-tab-pane name="summary" tab="版本摘要">
            <div class="summary-grid">
              <div><span>生成时间</span><strong>{{ new Date(store.draft.updatedAt).toLocaleString() }}</strong></div>
              <div><span>输出格式</span><strong>{{ isYaml ? 'Mihomo YAML' : 'Base64 URI 列表' }}</strong></div>
              <div><span>成功来源</span><strong>{{ store.subscriptions.filter(item => item.lastStatus === 'success').length }}</strong></div>
              <div><span>失败来源</span><strong>{{ store.draft.sourceFailures.length }}</strong></div>
            </div>
          </n-tab-pane>
          <n-tab-pane name="yaml" :tab="isYaml ? 'YAML 预览' : '订阅内容'" display-directive="show:lazy">
            <div class="yaml-toolbar">
              <file-code2 :size="16" /><span>可编辑 {{ contentLabel }}</span><n-tag size="small" :bordered="false">{{ yamlText.length.toLocaleString() }} 字符</n-tag>
              <div class="yaml-toolbar__actions">
                <n-button size="small" :disabled="!yamlDirty" @click="resetYaml"><template #icon><rotate-ccw :size="14" /></template>恢复</n-button>
                <n-button size="small" type="primary" :loading="savingYaml" :disabled="!yamlDirty" @click="saveYaml"><template #icon><save :size="14" /></template>保存</n-button>
              </div>
            </div>
            <n-input v-model:value="yamlText" type="textarea" class="yaml-editor mono" :input-props="{ spellcheck: false }" />
          </n-tab-pane>
        </n-tabs>
      </section>
    </template>
    <n-drawer v-model:show="historyDrawerVisible" placement="right" width="min(460px, 100vw)">
      <n-drawer-content title="草稿版本" closable>
        <div class="drawer-toolbar">
          <n-tag size="small" :bordered="false">{{ historyItems.length }} 个历史版本</n-tag>
          <n-popconfirm @positive-click="deleteOtherHistory"><template #trigger><n-button size="small" type="error" secondary :loading="deletingOtherHistory" :disabled="historyItems.filter(item => item.revision !== store.draft?.revision).length === 0"><template #icon><trash2 :size="15" /></template>删除其他草稿</n-button></template>此操作会永久删除当前草稿之外的全部历史版本，确认继续？</n-popconfirm>
        </div>
        <n-empty v-if="!historyLoading && historyItems.length === 0" description="暂无草稿历史" />
        <div v-else class="draft-history-list">
          <div v-for="item in historyItems" :key="item.id" class="draft-history-row">
            <div class="draft-history-row__meta">
              <div><strong>r{{ item.revision }}</strong><n-tag v-if="item.revision === store.draft?.revision" size="tiny" type="success" :bordered="false">当前草稿</n-tag></div>
              <span>{{ item.action }} · {{ new Date(item.createdAt).toLocaleString() }}</span>
              <span>{{ item.nodeCount }} 个节点 · {{ item.groupCount }} 个分组</span>
            </div>
            <div class="draft-history-row__actions">
              <n-button size="small" :loading="restoringHistoryId === item.id" :disabled="item.revision === store.draft?.revision" @click="restoreHistory(item.id)">切换</n-button>
              <n-popconfirm @positive-click="deleteHistory(item.id)"><template #trigger><n-button size="small" quaternary type="error" :loading="deletingHistoryId === item.id" :disabled="item.revision === store.draft?.revision" aria-label="删除草稿版本"><template #icon><trash2 :size="15" /></template></n-button></template>确认删除草稿版本 r{{ item.revision }}？</n-popconfirm>
            </div>
          </div>
        </div>
      </n-drawer-content>
    </n-drawer>
  </main>
</template>

<style scoped>
.preview-metrics { margin-bottom: 14px; }
.metric-button { position: relative; color: inherit; background: var(--mc-surface); border: 0; border-right: 1px solid var(--mc-border); cursor: pointer; text-align: left; }
.metric-button:hover { background: var(--mc-surface-muted); }
.metric-button > svg { position: absolute; top: 12px; right: 12px; color: var(--mc-primary); }
.preview-panel { min-height: 470px; overflow: hidden; }
.issue-row { display: grid; grid-template-columns: 82px 1fr; gap: 12px; align-items: start; }
.issue-row div { display: flex; flex-direction: column; }
.issue-row span { color: var(--mc-text-secondary); font-size: 12px; }
.summary-grid { display: grid; grid-template-columns: repeat(2, minmax(220px, 1fr)); gap: 1px; background: var(--mc-border); border: 1px solid var(--mc-border); }
.summary-grid div { min-height: 74px; padding: 12px; display: flex; flex-direction: column; background: var(--mc-surface); }
.summary-grid span { color: var(--mc-text-secondary); font-size: 12px; }
.summary-grid strong { margin-top: 5px; }
.yaml-toolbar { height: 36px; display: flex; align-items: center; gap: 8px; color: var(--mc-text-secondary); }
.yaml-toolbar__actions { margin-left: auto; display: flex; gap: 8px; }
.yaml-editor { min-height: 360px; }
.yaml-editor :deep(textarea) { min-height: 360px; font: 12px/1.55 ui-monospace, "Cascadia Code", Consolas, monospace; white-space: pre; }
.drawer-toolbar { min-height: 40px; display: flex; align-items: center; justify-content: space-between; gap: 8px; border-bottom: 1px solid var(--mc-border); }
.draft-history-list { overflow: auto; }
.draft-history-row { min-height: 76px; padding: 11px 0; display: flex; align-items: center; justify-content: space-between; gap: 12px; border-bottom: 1px solid var(--mc-border); }
.draft-history-row__meta { min-width: 0; display: flex; flex-direction: column; gap: 4px; }
.draft-history-row__meta > div { display: flex; align-items: center; gap: 8px; }
.draft-history-row__meta span { color: var(--mc-text-secondary); font-size: 12px; }
.draft-history-row__actions { display: flex; align-items: center; gap: 4px; }
</style>
