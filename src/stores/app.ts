import { computed, ref, shallowRef } from "vue";
import { defineStore } from "pinia";
import { api } from "@/services/api";
import type { Draft, ProjectSettings, PublishStatus, Subscription, TemplateSummary } from "@/types";

export const useAppStore = defineStore("app", () => {
  const initialized = ref(false);
  const loading = ref(false);
  const refreshing = ref(false);
  const templates = ref<TemplateSummary[]>([]);
  const settings = ref<ProjectSettings>({ templateId: "clash-mihomo", templateVersion: 1, mergeMode: "proxy-providers", theme: "system" });
  const subscriptions = ref<Subscription[]>([]);
  const draft = shallowRef<Draft | null>(null);
  const publishStatus = ref<PublishStatus>({ running: false, port: 17890, bindAddress: "0.0.0.0", lanAddresses: [], proxyDetected: false });

  const currentTemplate = computed(() => templates.value.find((item) => item.id === settings.value.templateId));
  const draftDirty = computed(() => Boolean(draft.value && (!draft.value.publishedAt || draft.value.updatedAt > draft.value.publishedAt)));
  const blockers = computed(() => draft.value?.issues.filter((item) => item.severity === "blocker") ?? []);
  const warnings = computed(() => draft.value?.issues.filter((item) => item.severity === "warning") ?? []);

  async function initialize() {
    if (initialized.value) return;
    loading.value = true;
    try {
      [templates.value, settings.value, subscriptions.value, draft.value, publishStatus.value] = await Promise.all([
        api.listTemplates(), api.getSettings(), api.listSubscriptions(), api.getDraft(), api.getPublishStatus(),
      ]);
      initialized.value = true;
    } finally {
      loading.value = false;
    }
  }

  async function reloadSubscriptions() {
    subscriptions.value = await api.listSubscriptions();
  }

  async function refreshAll() {
    refreshing.value = true;
    try {
      const result = await api.refreshSubscriptions();
      draft.value = result.draft;
      await reloadSubscriptions();
      return result;
    } finally {
      refreshing.value = false;
    }
  }

  async function selectTemplate(templateId: string, version: number, mergeMode: ProjectSettings["mergeMode"]) {
    settings.value = await api.selectTemplate(templateId, version, mergeMode);
    draft.value = await api.getDraft();
  }

  async function setTheme(theme: ProjectSettings["theme"]) {
    settings.value = await api.saveTheme(theme);
  }

  return {
    initialized, loading, refreshing, templates, settings, subscriptions, draft, publishStatus,
    currentTemplate, draftDirty, blockers, warnings,
    initialize, reloadSubscriptions, refreshAll, selectTemplate, setTheme,
  };
});
