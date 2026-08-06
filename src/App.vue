<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import {
  NConfigProvider, NDialogProvider, NGlobalStyle, NMessageProvider,
  NNotificationProvider, darkTheme, type GlobalThemeOverrides,
} from "naive-ui";
import AppShell from "@/layouts/AppShell.vue";
import { useAppStore } from "@/stores/app";

const store = useAppStore();
const systemDark = ref(matchMedia("(prefers-color-scheme: dark)").matches);

const isDark = computed(() => store.settings.theme === "dark" || (store.settings.theme === "system" && systemDark.value));
const themeOverrides = computed<GlobalThemeOverrides>(() => ({
  common: isDark.value ? {
    primaryColor: "#2DD4BF", primaryColorHover: "#5EEAD4", primaryColorPressed: "#14B8A6",
    infoColor: "#60A5FA", successColor: "#4ADE80", warningColor: "#FBBF24", errorColor: "#F87171",
    bodyColor: "#18181B", cardColor: "#242427", modalColor: "#242427", popoverColor: "#242427",
    textColorBase: "#FAFAFA", textColor2: "#D4D4D8", textColor3: "#A1A1AA", borderColor: "#3F3F46",
    borderRadius: "4px", fontSize: "14px", fontFamily: 'system-ui, "Segoe UI", "Microsoft YaHei UI", sans-serif',
    fontFamilyMono: 'ui-monospace, "Cascadia Code", Consolas, monospace',
  } : {
    primaryColor: "#0F766E", primaryColorHover: "#115E59", primaryColorPressed: "#134E4A",
    infoColor: "#2563EB", successColor: "#15803D", warningColor: "#B45309", errorColor: "#B91C1C",
    bodyColor: "#F6F7F9", cardColor: "#FFFFFF", modalColor: "#FFFFFF", popoverColor: "#FFFFFF",
    textColorBase: "#18181B", textColor2: "#3F3F46", textColor3: "#52525B", borderColor: "#D4D4D8",
    borderRadius: "4px", fontSize: "14px", fontFamily: 'system-ui, "Segoe UI", "Microsoft YaHei UI", sans-serif',
    fontFamilyMono: 'ui-monospace, "Cascadia Code", Consolas, monospace',
  },
  Button: { heightMedium: "34px", heightSmall: "30px", borderRadiusMedium: "4px", borderRadiusSmall: "4px" },
  Input: { heightMedium: "34px", heightSmall: "30px", borderRadius: "4px" },
  Select: { peers: { InternalSelection: { heightMedium: "34px", heightSmall: "30px" } } },
  Card: { borderRadius: "6px" },
  Modal: { borderRadius: "8px" },
  DataTable: { thColor: isDark.value ? "#303034" : "#EEF1F4", thColorHover: isDark.value ? "#343438" : "#E7EAEE", tdColorHover: isDark.value ? "#2B2B2F" : "#F4F6F8", borderColor: isDark.value ? "#3F3F46" : "#D4D4D8" },
}));

watch(isDark, (value) => document.documentElement.dataset.theme = value ? "dark" : "light", { immediate: true });
onMounted(() => {
  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", (event) => systemDark.value = event.matches);
  store.initialize();
});
</script>

<template>
  <n-config-provider :theme="isDark ? darkTheme : null" :theme-overrides="themeOverrides">
    <n-global-style />
    <n-dialog-provider>
      <n-message-provider>
        <n-notification-provider>
          <app-shell />
        </n-notification-provider>
      </n-message-provider>
    </n-dialog-provider>
  </n-config-provider>
</template>
