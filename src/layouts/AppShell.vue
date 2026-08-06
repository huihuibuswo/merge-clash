<script setup lang="ts">
import { computed, h, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { NIcon, NLayout, NLayoutContent, NLayoutHeader, NLayoutSider, NMenu, NSpin } from "naive-ui";
import {
  FileCode2, LayoutDashboard, Link2, Network, PanelsTopLeft, RadioTower, Settings,
} from "lucide-vue-next";
import { useAppStore } from "@/stores/app";
import StatusLabel from "@/components/StatusLabel.vue";

const route = useRoute();
const router = useRouter();
const store = useAppStore();
const collapsed = ref(false);
const renderIcon = (icon: typeof LayoutDashboard) => () => h(NIcon, null, { default: () => h(icon, { size: 18, strokeWidth: 1.75 }) });
const menuOptions = [
  { label: "概览", key: "/", icon: renderIcon(LayoutDashboard) },
  { label: "配置模板", key: "/templates", icon: renderIcon(PanelsTopLeft) },
  { label: "订阅源", key: "/subscriptions", icon: renderIcon(Link2) },
  { label: "节点与分组", key: "/groups", icon: renderIcon(Network) },
  { label: "配置预览", key: "/preview", icon: renderIcon(FileCode2) },
  { label: "本地发布", key: "/publishing", icon: renderIcon(RadioTower) },
  { type: "divider", key: "divider" },
  { label: "设置", key: "/settings", icon: renderIcon(Settings) },
];
const activeKey = computed(() => route.path);
</script>

<template>
  <n-layout class="app-shell" has-sider>
    <n-layout-sider
      bordered collapse-mode="width" :collapsed-width="56" :width="216"
      :collapsed="collapsed" show-trigger="bar" @collapse="collapsed = true" @expand="collapsed = false"
    >
      <div class="brand" :class="{ 'brand--collapsed': collapsed }">
        <div class="brand__mark">MC</div>
        <div v-if="!collapsed" class="brand__text"><strong>Merge Clash</strong><span>订阅配置工作台</span></div>
      </div>
      <n-menu :value="activeKey" :options="menuOptions" :collapsed="collapsed" :collapsed-width="56" :collapsed-icon-size="18" @update:value="(key) => router.push(String(key))" />
    </n-layout-sider>
    <n-layout>
      <n-layout-header class="topbar" bordered>
        <div class="topbar__project">
          <strong>{{ store.currentTemplate?.name ?? "Merge Clash" }}</strong>
          <span>{{ store.currentTemplate?.outputFormat === 'mihomo-yaml' ? (store.settings.mergeMode === 'proxy-providers' ? '动态 Provider' : '静态节点') : 'Base64 URI 订阅' }}</span>
        </div>
        <div class="topbar__status">
          <status-label v-if="store.draftDirty" status="warning" text="草稿未发布" />
          <status-label v-else status="success" text="配置已同步" />
          <status-label :status="store.publishStatus.running ? 'success' : 'never'" :text="store.publishStatus.running ? '服务运行中' : '服务未启动'" />
        </div>
      </n-layout-header>
      <n-layout-content class="content" :native-scrollbar="false">
        <div v-if="store.loading" class="page-loading"><n-spin size="medium" /><span>正在加载本地数据</span></div>
        <router-view v-else />
      </n-layout-content>
    </n-layout>
  </n-layout>
</template>
