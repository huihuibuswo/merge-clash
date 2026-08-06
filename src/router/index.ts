/*
 * @Author: huihuibuswo linqsong@126.com
 * @Date: 2026-08-03 16:16:31
 * @LastEditors: huihuibuswo linqsong@126.com
 * @LastEditTime: 2026-08-04 15:12:08
 * @FilePath: \merge-clash\src\router\index.ts
 * @Description: 这是默认设置,请设置`customMade`, 打开koroFileHeader查看配置 进行设置: https://github.com/OBKoro1/koro1FileHeader/wiki/%E9%85%8D%E7%BD%AE
 */
import { createRouter, createWebHashHistory } from "vue-router";

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    {
      path: "/",
      name: "overview",
      component: () => import("@/views/OverviewView.vue"),
      meta: { title: "概览" },
    },
    {
      path: "/templates",
      name: "templates",
      component: () => import("@/views/TemplatesView.vue"),
      meta: { title: "配置模板" },
    },
    {
      path: "/subscriptions",
      name: "subscriptions",
      component: () => import("@/views/SubscriptionsView.vue"),
      meta: { title: "订阅源" },
    },
    {
      path: "/groups",
      name: "groups",
      component: () => import("@/views/GroupsView.vue"),
      meta: { title: "节点与分组" },
    },
    {
      path: "/preview",
      name: "preview",
      component: () => import("@/views/PreviewView.vue"),
      meta: { title: "配置预览" },
    },
    {
      path: "/publishing",
      name: "publishing",
      component: () => import("@/views/PublishingView.vue"),
      meta: { title: "本地发布" },
    },
    {
      path: "/settings",
      name: "settings",
      component: () => import("@/views/SettingsView.vue"),
      meta: { title: "设置" },
    },
  ],
});

router.afterEach((to) => {
  document.title = `${String(to.meta.title)} - Merge Clash`;
  // requestAnimationFrame(() => document.querySelector<HTMLElement>("#page-title")?.focus());
});

export default router;
