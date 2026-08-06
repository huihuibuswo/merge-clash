<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { NAlert, NButton, NDrawer, NDrawerContent, NEmpty, NForm, NFormItem, NInput, NInputNumber, NPopconfirm, NSelect, NTag, useMessage } from "naive-ui";
import { Check, Copy, History, Play, QrCode, RadioTower, RotateCcw, Square, Trash2 } from "lucide-vue-next";
import QRCode from "qrcode";
import PageHeader from "@/components/PageHeader.vue";
import StatusLabel from "@/components/StatusLabel.vue";
import { api } from "@/services/api";
import { useAppStore } from "@/stores/app";
import type { PublishedVersion } from "@/types";

const store = useAppStore();
const message = useMessage();
const busy = ref(false);
const port = ref(store.publishStatus.port || 17890);
const qrData = ref("");
const versions = ref<PublishedVersion[]>([]);
const versionDrawerVisible = ref(false);
const selectedVersion = ref<number | null>(store.publishStatus.versionNo ?? null);
const switchingVersion = ref(false);
const deletingVersion = ref<number | null>(null);
const deletingOtherVersions = ref(false);
const url = computed(() => store.publishStatus.subscriptionUrl ?? "");
const maskedUrl = computed(() => url.value.replace(/(\/subscription\/)[^/]+(\/config)/, "$1***$2"));
const versionOptions = computed(() => versions.value.map((item) => ({
  label: `v${item.versionNo} · ${new Date(item.createdAt).toLocaleString()}`,
  value: item.versionNo,
})));
watch(url, async (value) => { qrData.value = value ? await QRCode.toDataURL(value, { width: 220, margin: 1, color: { dark: "#18181B", light: "#FFFFFF" } }) : ""; }, { immediate: true });
watch(() => store.publishStatus.versionNo, (value) => { selectedVersion.value = value ?? null; });
onMounted(async () => {
  [store.publishStatus, versions.value] = await Promise.all([api.getPublishStatus(), api.listPublishedVersions()]);
  port.value = store.publishStatus.port;
  selectedVersion.value = store.publishStatus.versionNo ?? null;
});
async function toggleServer() {
  busy.value = true;
  try {
    store.publishStatus = store.publishStatus.running ? await api.stopServer() : await api.startServer();
    message.success(store.publishStatus.running ? "局域网服务已启动" : "局域网服务已停止");
  } finally { busy.value = false; }
}
async function savePort() { store.publishStatus = await api.savePublishSettings(port.value); message.success("端口设置已保存"); }
async function copyUrl() { if (!url.value) return; await navigator.clipboard.writeText(url.value); message.success("订阅地址已复制"); }
async function rotateToken() { store.publishStatus = await api.rotateToken(); message.success("访问令牌已重置，旧地址已失效"); }
async function openVersionDrawer() {
  versionDrawerVisible.value = true;
  versions.value = await api.listPublishedVersions();
}
async function activateVersion() {
  if (!selectedVersion.value || selectedVersion.value === store.publishStatus.versionNo) return;
  switchingVersion.value = true;
  try {
    const wasRunning = store.publishStatus.running;
    store.publishStatus = await api.activatePublishedVersion(selectedVersion.value);
    versions.value = await api.listPublishedVersions();
    message.success(`${wasRunning ? "服务已停止，" : ""}已切换到发布版本 v${selectedVersion.value}`);
  } finally { switchingVersion.value = false; }
}
async function deleteVersion(versionNo: number) {
  deletingVersion.value = versionNo;
  try {
    store.publishStatus = await api.deletePublishedVersion(versionNo);
    versions.value = await api.listPublishedVersions();
    selectedVersion.value = store.publishStatus.versionNo ?? null;
    message.success(`已删除发布版本 v${versionNo}`);
  } finally { deletingVersion.value = null; }
}
async function deleteOtherVersions() {
  deletingOtherVersions.value = true;
  try {
    versions.value = await api.deleteOtherPublishedVersions();
    selectedVersion.value = store.publishStatus.versionNo ?? null;
    message.success("已删除当前发布版本之外的全部版本");
  } finally { deletingOtherVersions.value = false; }
}
</script>

<template>
  <main class="page">
    <page-header title="本地发布" description="向同一专用局域网中的手机提供已发布配置">
      <n-button :type="store.publishStatus.running ? 'error' : 'primary'" :loading="busy" :disabled="!store.publishStatus.lastPublishedAt" @click="toggleServer">
        <template #icon><square v-if="store.publishStatus.running" :size="15" /><play v-else :size="15" /></template>{{ store.publishStatus.running ? '停止服务' : '启动服务' }}
      </n-button>
    </page-header>
    <section class="surface publish-layout">
      <div class="publish-main">
        <div class="publish-status"><div><radio-tower :size="24" /><div><strong>服务状态</strong><status-label :status="store.publishStatus.running ? 'success' : 'never'" :text="store.publishStatus.running ? '运行中' : '未启动'" /></div></div><n-button text class="version-trigger" @click="openVersionDrawer"><template #icon><history :size="15" /></template>{{ store.publishStatus.versionNo ? `发布版本 v${store.publishStatus.versionNo}` : '发布版本' }}</n-button></div>
        <n-form label-placement="left" label-width="88" style="max-width:560px">
          <n-form-item label="监听地址"><n-input :value="store.publishStatus.bindAddress" readonly class="mono" /></n-form-item>
          <n-form-item label="端口"><div style="display:flex;gap:8px"><n-input-number v-model:value="port" :min="1024" :max="65535" class="mono" /><n-button @click="savePort">保存</n-button></div></n-form-item>
          <n-form-item label="局域网地址"><n-input :value="store.publishStatus.lanAddresses.join('、') || '未检测到局域网地址'" readonly class="mono" /></n-form-item>
        </n-form>
        <n-alert v-if="store.publishStatus.proxyDetected" type="info" style="margin-top:14px">已检测到系统代理或代理网卡，发布地址已使用真实局域网 IP。</n-alert>
        <n-alert v-if="!store.publishStatus.lastPublishedAt" type="warning" style="margin-top:14px">尚未发布有效草稿，请先在“配置预览”发布当前版本。</n-alert>
        <template v-else>
          <div class="subscription-address"><div class="subscription-address__label"><strong>手机订阅地址</strong><span>令牌已脱敏显示，复制时使用完整地址</span></div><div class="subscription-address__field"><code class="subscription-address__code mono">{{ maskedUrl || '启动服务后生成地址' }}</code><n-button :disabled="!url" aria-label="复制订阅地址" @click="copyUrl"><template #icon><copy :size="16" /></template>复制</n-button></div></div>
          <n-alert type="info" :bordered="false">仅建议在可信的专用网络中开启。动态 Provider 配置包含原始订阅 URL，静态配置和 URI 订阅包含节点凭据。</n-alert>
        </template>
      </div>
      <aside class="qr-panel">
        <div class="qr-box"><img v-if="qrData" :src="qrData" alt="手机订阅地址二维码" width="220" height="220" /><div v-else><qr-code :size="36" /><span>启动服务后生成二维码</span></div></div>
        <span class="muted">手机与电脑需连接同一个局域网</span>
      </aside>
    </section>
    <section class="danger-zone">
      <h2>危险操作</h2><p class="muted">重置访问令牌后，手机中保存的旧订阅地址立即失效。</p>
      <n-popconfirm @positive-click="rotateToken"><template #trigger><n-button type="error" secondary :disabled="!store.publishStatus.lastPublishedAt"><template #icon><rotate-ccw :size="16" /></template>重置访问令牌</n-button></template>确认重置令牌并使旧地址失效？</n-popconfirm>
    </section>
    <n-drawer v-model:show="versionDrawerVisible" placement="right" width="min(460px, 100vw)">
      <n-drawer-content title="发布版本" closable>
        <div class="drawer-toolbar">
          <n-tag size="small" :bordered="false">{{ versions.length }} 个版本</n-tag>
          <n-popconfirm @positive-click="deleteOtherVersions"><template #trigger><n-button size="small" type="error" secondary :loading="deletingOtherVersions" :disabled="versions.filter(item => !item.active).length === 0"><template #icon><trash2 :size="15" /></template>删除其他版本</n-button></template>此操作会永久删除当前发布版本之外的全部版本，确认继续？</n-popconfirm>
        </div>
        <div class="version-picker">
          <n-select v-model:value="selectedVersion" :options="versionOptions" placeholder="选择要发布的版本" :disabled="versions.length === 0" />
          <n-button type="primary" :loading="switchingVersion" :disabled="!selectedVersion || selectedVersion === store.publishStatus.versionNo" @click="activateVersion"><template #icon><check :size="16" /></template>切换版本</n-button>
        </div>
        <div v-if="versions.length" class="version-list">
          <div v-for="item in versions" :key="item.versionNo" class="version-row">
            <div class="version-row__meta">
              <div><strong>v{{ item.versionNo }}</strong><n-tag v-if="item.active" size="tiny" type="success" :bordered="false">当前发布</n-tag></div>
              <span>{{ new Date(item.createdAt).toLocaleString() }} · {{ item.templateId }} v{{ item.templateVersion }}</span>
              <span>{{ item.mergeMode === 'proxy-providers' ? '动态 Provider' : '静态节点' }}</span>
            </div>
            <n-popconfirm @positive-click="deleteVersion(item.versionNo)"><template #trigger><n-button size="small" quaternary type="error" :loading="deletingVersion === item.versionNo" aria-label="删除发布版本"><template #icon><trash2 :size="15" /></template></n-button></template>{{ item.active ? '删除当前版本后将自动切换到最新剩余版本。' : '' }}确认删除发布版本 v{{ item.versionNo }}？</n-popconfirm>
          </div>
        </div>
        <n-empty v-else description="暂无已发布版本" />
      </n-drawer-content>
    </n-drawer>
  </main>
</template>

<style scoped>
.publish-layout { display: grid; grid-template-columns: minmax(520px, 1fr) 300px; overflow: hidden; }
.publish-main { padding: 16px; }
.publish-status { min-height: 58px; margin-bottom: 18px; display: flex; align-items: flex-start; justify-content: space-between; border-bottom: 1px solid var(--mc-border); }
.publish-status > div { display: flex; gap: 10px; }
.publish-status > div > div { display: flex; flex-direction: column; gap: 4px; }
.version-trigger { color: var(--mc-primary); }
.subscription-address { margin: 18px 0 12px; }
.subscription-address__label { display: flex; align-items: baseline; justify-content: space-between; margin-bottom: 7px; }
.subscription-address__label span { color: var(--mc-text-secondary); font-size: 12px; }
.subscription-address__field { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 8px; align-items: center; }
.subscription-address__code { min-width: 0; overflow-wrap: anywhere; color: var(--mc-text-secondary); }
.qr-panel { padding: 20px; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px; border-left: 1px solid var(--mc-border); background: var(--mc-surface-muted); text-align: center; }
.qr-box { width: 222px; height: 222px; display: grid; place-items: center; background: #fff; border: 1px solid var(--mc-border); }
.qr-box > div { display: flex; flex-direction: column; align-items: center; gap: 10px; color: var(--mc-text-secondary); }
.drawer-toolbar { min-height: 40px; display: flex; align-items: center; justify-content: space-between; gap: 8px; border-bottom: 1px solid var(--mc-border); }
.version-picker { padding: 12px 0; display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 8px; align-items: center; border-bottom: 1px solid var(--mc-border); }
.version-list { overflow: auto; }
.version-row { min-height: 62px; padding: 10px 12px; display: flex; align-items: center; justify-content: space-between; gap: 16px; border-bottom: 1px solid var(--mc-border); }
.version-row:last-child { border-bottom: 0; }
.version-row__meta { min-width: 0; display: flex; flex-direction: column; gap: 4px; }
.version-row__meta > div { display: flex; align-items: center; gap: 8px; }
.version-row__meta span { color: var(--mc-text-secondary); font-size: 12px; }
.danger-zone h2 { margin: 0 0 4px; font-size: 15px; }
.danger-zone p { margin: 0 0 12px; }
@media (max-width: 1199px) {
  .publish-layout { grid-template-columns: 1fr; }
  .qr-panel { border-top: 1px solid var(--mc-border); border-left: 0; }
}
</style>
