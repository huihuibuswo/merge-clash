<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { NAlert, NButton, NDescriptions, NDescriptionsItem, NRadio, NRadioButton, NRadioGroup, NTag, useDialog, useMessage } from "naive-ui";
import { FileSearch, PanelsTopLeft } from "lucide-vue-next";
import PageHeader from "@/components/PageHeader.vue";
import { useAppStore } from "@/stores/app";
import type { MergeMode } from "@/types";

const store = useAppStore();
const dialog = useDialog();
const message = useMessage();
const selectedId = ref(store.settings.templateId);
const selectedMode = ref<MergeMode>(store.settings.mergeMode);
watch(() => store.settings.templateId, (value) => selectedId.value = value);
const selected = computed(() => store.templates.find((item) => item.id === selectedId.value) ?? store.templates[0]);
watch(selected, (value) => {
  if (value && !value.supportedModes.includes(selectedMode.value)) selectedMode.value = value.defaultMode;
});
const modeOptions = computed(() => selected.value?.supportedModes.map((mode) => ({
  label: mode === "proxy-providers" ? "动态 Provider" : "静态节点",
  value: mode,
})) ?? []);
const hasChange = computed(() => selectedId.value !== store.settings.templateId || selectedMode.value !== store.settings.mergeMode);

function applyTemplate() {
  if (!selected.value || !hasChange.value) return;
  dialog.warning({
    title: "重建当前草稿",
    content: "切换模板或合并模式会重建当前草稿，未发布的内容可能被替换。已发布版本不受影响。",
    positiveText: "确认切换",
    negativeText: "取消",
    async onPositiveClick() {
      await store.selectTemplate(selected.value!.id, selected.value!.version, selectedMode.value);
      message.success(`已切换到 ${selected.value!.name}`);
    },
  });
}
</script>

<template>
  <main class="page">
    <page-header title="配置模板" description="选择目标客户端与输出格式，输入订阅仍需为 Clash/Mihomo YAML">
      <n-button type="primary" :disabled="!hasChange" @click="applyTemplate">使用此模板</n-button>
    </page-header>

    <div class="template-layout">
      <section class="surface template-list" aria-label="模板列表">
        <button v-for="item in store.templates" :key="item.id" class="template-item" :class="{ 'template-item--active': selectedId === item.id }" @click="selectedId = item.id">
          <n-radio :checked="selectedId === item.id" :aria-label="`选择 ${item.name}`" />
          <div class="template-item__icon"><panels-top-left :size="20" /></div>
          <div class="template-item__content">
            <div><strong>{{ item.name }}</strong><n-tag v-if="store.settings.templateId === item.id" size="small" type="success" :bordered="false">当前使用</n-tag></div>
            <span>{{ item.core }} · {{ item.outputFormat === 'mihomo-yaml' ? 'YAML 配置' : 'Base64 URI 订阅' }}</span>
          </div>
        </button>
      </section>

      <section v-if="selected" class="surface template-detail">
        <div class="template-detail__header">
          <div><h2>{{ selected.name }}</h2><p>{{ selected.description }}</p></div>
          <n-tag size="small" :bordered="false">v{{ selected.version }}</n-tag>
        </div>
        <div class="template-detail__body">
          <label class="field-label">合并模式</label>
          <n-radio-group v-model:value="selectedMode" size="small">
            <n-radio-button v-for="option in modeOptions" :key="option.value" :value="option.value">{{ option.label }}</n-radio-button>
          </n-radio-group>
          <n-alert style="margin-top:12px" type="info" :bordered="false">
            {{ selectedMode === 'proxy-providers' ? '客户端直接刷新各原始订阅，生成文件会包含原始订阅 URL。' : selected.outputFormat === 'mihomo-yaml' ? '桌面端展开、清洗并去重节点，生成文件不包含原始订阅 URL。' : '输入源仍需为 Clash/Mihomo YAML；输出会转换为客户端可导入的 Base64 分享链接订阅。' }}
          </n-alert>
          <n-descriptions style="margin-top:18px" label-placement="left" :column="1" bordered size="small">
            <n-descriptions-item label="核心">{{ selected.core }}</n-descriptions-item>
            <n-descriptions-item label="输出格式">{{ selected.outputFormat === 'mihomo-yaml' ? 'Mihomo YAML' : 'Base64 URI 列表' }}</n-descriptions-item>
            <n-descriptions-item label="文件名"><span class="mono">{{ selected.fileName }}</span></n-descriptions-item>
            <n-descriptions-item v-if="selected.groups.length" label="代理组"><span class="tag-row"><n-tag v-for="group in selected.groups" :key="group" size="small">{{ group }}</n-tag></span></n-descriptions-item>
            <n-descriptions-item label="外部依赖">{{ selected.externalDependencies.join('、') || '无' }}</n-descriptions-item>
          </n-descriptions>
          <div class="template-preview-placeholder">
            <file-search :size="28" /><div><strong>模板生成预览</strong><span>刷新订阅后可在“配置预览”查看并编辑完整输出内容。</span></div>
          </div>
        </div>
      </section>
    </div>
  </main>
</template>

<style scoped>
.template-layout { display: grid; grid-template-columns: 340px minmax(480px, 1fr); gap: 14px; min-height: 520px; }
.template-list { padding: 6px; }
.template-item { width: 100%; min-height: 76px; display: grid; grid-template-columns: 22px 38px 1fr; align-items: center; gap: 9px; padding: 10px; color: inherit; text-align: left; background: transparent; border: 1px solid transparent; border-radius: 4px; cursor: pointer; }
.template-item:hover { background: var(--mc-surface-muted); }
.template-item--active { background: color-mix(in srgb, var(--mc-primary) 10%, transparent); border-color: var(--mc-primary); }
.template-item__icon { width: 34px; height: 34px; display: grid; place-items: center; color: var(--mc-primary); background: var(--mc-surface-muted); border-radius: 4px; }
.template-item__content { min-width: 0; }
.template-item__content > div { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.template-item__content span { color: var(--mc-text-secondary); font-size: 12px; }
.template-detail__header { min-height: 76px; padding: 14px 16px; display: flex; align-items: flex-start; justify-content: space-between; border-bottom: 1px solid var(--mc-border); }
.template-detail__header h2 { margin: 0 0 4px; font-size: 16px; }
.template-detail__header p { margin: 0; color: var(--mc-text-secondary); }
.template-detail__body { padding: 16px; }
.field-label { display: block; margin-bottom: 6px; font-weight: 500; }
.tag-row { display: flex; flex-wrap: wrap; gap: 6px; }
.template-preview-placeholder { margin-top: 18px; min-height: 110px; display: flex; align-items: center; justify-content: center; gap: 12px; color: var(--mc-text-secondary); border: 1px dashed var(--mc-border); border-radius: 4px; }
.template-preview-placeholder div { display: flex; flex-direction: column; }
@media (max-width: 1050px) { .template-layout { grid-template-columns: 290px 1fr; } }
</style>
