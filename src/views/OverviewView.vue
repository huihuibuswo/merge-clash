<script setup lang="ts">
import { computed } from "vue";
import { useRouter } from "vue-router";
import { NAlert, NButton, NEmpty, NList, NListItem, NThing, useMessage } from "naive-ui";
import { RefreshCw } from "lucide-vue-next";
import PageHeader from "@/components/PageHeader.vue";
import StatusLabel from "@/components/StatusLabel.vue";
import { useAppStore } from "@/stores/app";

const store = useAppStore();
const router = useRouter();
const message = useMessage();
const successful = computed(() => store.subscriptions.filter((item) => item.lastStatus === "success").length);
const issues = computed(() => store.draft?.issues.filter((item) => item.severity !== "info").slice(0, 5) ?? []);

async function refresh() {
  const result = await store.refreshAll();
  message.success(`刷新完成：${result.successful} 个成功，${result.failed} 个失败`);
}
</script>

<template>
  <main class="page">
    <page-header title="概览" description="订阅、草稿和局域网服务的当前状态">
      <n-button type="primary" :loading="store.refreshing" :disabled="store.subscriptions.length === 0" @click="refresh">
        <template #icon><refresh-cw :size="16" /></template>刷新全部
      </n-button>
    </page-header>

    <section class="metric-strip" aria-label="运行摘要">
      <div class="metric"><span>当前模板</span><strong style="font-size:14px">{{ store.currentTemplate?.name ?? '-' }}</strong></div>
      <div class="metric"><span>订阅成功</span><strong>{{ successful }}/{{ store.subscriptions.length }}</strong></div>
      <div class="metric"><span>节点</span><strong>{{ store.draft?.proxies.length ?? 0 }}</strong></div>
      <div class="metric"><span>代理组</span><strong>{{ store.draft?.groups.length ?? 0 }}</strong></div>
      <div class="metric"><span>阻断</span><strong>{{ store.blockers.length }}</strong></div>
      <div class="metric"><span>警告</span><strong>{{ store.warnings.length }}</strong></div>
    </section>

    <section class="surface section">
      <div class="section__header"><h2>需要处理</h2><n-button text size="small" @click="router.push('/preview')">查看完整校验</n-button></div>
      <div v-if="issues.length" class="section__body" style="padding:0 12px">
        <n-list>
          <n-list-item v-for="issue in issues" :key="`${issue.code}-${issue.target}`">
            <n-thing :title="issue.message" :description="issue.target || undefined">
              <template #avatar><status-label :status="issue.severity === 'blocker' ? 'error' : 'warning'" :text="issue.severity === 'blocker' ? '阻断' : '警告'" /></template>
            </n-thing>
          </n-list-item>
        </n-list>
      </div>
      <div v-else class="section__body"><n-empty description="当前没有阻断问题" /></div>
    </section>

    <section class="surface section">
      <div class="section__header"><h2>本地发布</h2><n-button text size="small" @click="router.push('/publishing')">打开详情</n-button></div>
      <div class="section__body">
        <n-alert :type="store.publishStatus.running ? 'success' : 'default'" :bordered="false">
          <template #header>{{ store.publishStatus.running ? '局域网订阅服务正在运行' : '局域网订阅服务尚未启动' }}</template>
          {{ store.publishStatus.running ? (store.publishStatus.subscriptionUrl ?? '服务已启动') : '发布草稿后，可在同一局域网内向手机提供订阅。' }}
        </n-alert>
      </div>
    </section>
  </main>
</template>
