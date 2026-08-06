<script setup lang="ts">
import { CircleCheck, CircleX, Clock3, LoaderCircle, TriangleAlert } from "lucide-vue-next";

const props = defineProps<{ status: "success" | "error" | "warning" | "running" | "never"; text?: string }>();
const labels = { success: "成功", error: "失败", warning: "警告", running: "运行中", never: "未运行" };
const icons = { success: CircleCheck, error: CircleX, warning: TriangleAlert, running: LoaderCircle, never: Clock3 };
</script>

<template>
  <span class="status-label" :class="`status-label--${status}`">
    <component :is="icons[props.status]" :size="15" :class="{ spin: status === 'running' }" aria-hidden="true" />
    {{ text ?? labels[props.status] }}
  </span>
</template>
