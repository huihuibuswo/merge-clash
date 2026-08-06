<script setup lang="ts">
import { computed } from "vue";
import { NAlert, NForm, NFormItem, NRadioButton, NRadioGroup, NTag, useMessage } from "naive-ui";
import PageHeader from "@/components/PageHeader.vue";
import { useAppStore } from "@/stores/app";
import type { ThemeMode } from "@/types";

const store = useAppStore();
const message = useMessage();
const theme = computed({ get: () => store.settings.theme, set: async (value: ThemeMode) => { await store.setTheme(value); message.success("外观设置已保存"); } });
</script>

<template>
  <main class="page">
    <page-header title="设置" description="外观、网络与本地数据行为" />
    <section class="settings-section">
      <h2>外观</h2><p>主题会立即应用，并在下次启动时恢复。</p>
      <n-form label-placement="left" label-width="120">
        <n-form-item label="界面主题"><n-radio-group v-model:value="theme"><n-radio-button value="system">跟随系统</n-radio-button><n-radio-button value="light">浅色</n-radio-button><n-radio-button value="dark">深色</n-radio-button></n-radio-group></n-form-item>
      </n-form>
    </section>
    <section class="settings-section">
      <h2>网络安全</h2><p>这些限制由 Rust 后端统一执行，前端无法绕过。</p>
      <div class="settings-list"><div><span>订阅请求超时</span><strong>30 秒</strong></div><div><span>最大响应体</span><strong>20 MiB</strong></div><div><span>允许协议</span><strong>HTTP / HTTPS</strong></div><div><span>URL 与凭据日志</span><n-tag size="small" type="success" :bordered="false">始终脱敏</n-tag></div></div>
    </section>
    <section class="settings-section">
      <h2>本地数据</h2><p>SQLite 数据库保存在操作系统应用数据目录，卸载应用通常不会自动删除。</p>
      <n-alert type="info" :bordered="false">订阅 URL、节点凭据和访问令牌属于敏感数据。MVP 依赖操作系统用户目录权限，不宣称数据库已加密。</n-alert>
    </section>
  </main>
</template>

<style scoped>
.settings-section { padding: 18px 0; border-bottom: 1px solid var(--mc-border); }
.settings-section:first-of-type { padding-top: 4px; }
.settings-section h2 { margin: 0 0 4px; font-size: 15px; }
.settings-section > p { margin: 0 0 16px; color: var(--mc-text-secondary); }
.settings-list { max-width: 660px; border: 1px solid var(--mc-border); border-radius: 4px; overflow: hidden; }
.settings-list > div { min-height: 42px; padding: 0 12px; display: flex; align-items: center; justify-content: space-between; border-bottom: 1px solid var(--mc-border); }
.settings-list > div:last-child { border-bottom: 0; }
</style>
