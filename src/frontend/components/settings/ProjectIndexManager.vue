<script setup lang="ts">
import type { ProjectIndexStatus } from '../../types/tauri'
import { invoke } from '@tauri-apps/api/core'
import { useDialog, useMessage } from 'naive-ui'
import { computed, onMounted, ref } from 'vue'
import { useAcemcpSync } from '../../composables/useAcemcpSync'
import McpIndexStatusDrawer from '../popup/McpIndexStatusDrawer.vue'

// 使用 Acemcp 同步状态管理
const { triggerIndexUpdate } = useAcemcpSync()

const message = useMessage()
const dialog = useDialog()

// 本地状态
const loading = ref(true)
const allProjects = ref<Record<string, ProjectIndexStatus>>({})
const watchingProjects = ref<string[]>([])
const selectedProject = ref<string>('')
const showDrawer = ref(false)
const resyncLoading = ref(false)

// 选中项目的状态信息（用于抽屉组件）
const selectedProjectStatus = computed<ProjectIndexStatus | null>(() => {
  if (!selectedProject.value)
    return null
  return allProjects.value[selectedProject.value] || null
})

// 选中项目的状态摘要文本
const selectedStatusSummary = computed(() => {
  const status = selectedProjectStatus.value
  if (!status)
    return '未索引'
  switch (status.status) {
    case 'idle':
      return '空闲'
    case 'indexing':
      return `索引中 ${status.progress}%`
    case 'synced':
      return '已同步'
    case 'failed':
      return '索引失败'
    default:
      return '未知状态'
  }
})

// 选中项目的状态图标
const selectedStatusIcon = computed(() => {
  const status = selectedProjectStatus.value?.status
  switch (status) {
    case 'idle':
      return 'i-carbon-circle-dash text-gray-400'
    case 'indexing':
      return 'i-carbon-in-progress text-blue-500 animate-spin'
    case 'synced':
      return 'i-carbon-checkmark-filled text-green-500'
    case 'failed':
      return 'i-carbon-warning-filled text-red-500'
    default:
      return 'i-carbon-help text-gray-400'
  }
})

// 选中项目是否正在索引
const selectedIsIndexing = computed(() => {
  return selectedProjectStatus.value?.status === 'indexing'
})

// 计算项目列表（转换为数组便于渲染）
const projectList = computed(() => {
  return Object.values(allProjects.value).sort((a, b) => {
    // 按状态排序：索引中 > 已完成 > 失败 > 未索引
    const statusOrder = { indexing: 0, synced: 1, failed: 2, idle: 3 }
    return statusOrder[a.status] - statusOrder[b.status]
  })
})

// 初始化加载
onMounted(async () => {
  await loadAllData()
})

// 加载所有数据
async function loadAllData() {
  loading.value = true
  try {
    // 并行加载所有项目状态和监听列表
    const [statusResult, watchingResult] = await Promise.all([
      invoke<{ projects: Record<string, ProjectIndexStatus> }>('get_all_acemcp_index_status'),
      invoke<string[]>('get_watching_projects'),
    ])
    allProjects.value = statusResult.projects
    watchingProjects.value = watchingResult
  }
  catch (err) {
    console.error('加载项目索引数据失败:', err)
    message.error('加载项目索引数据失败')
  }
  finally {
    loading.value = false
  }
}

// 复制项目路径
async function copyPath(path: string) {
  try {
    await navigator.clipboard.writeText(path)
    message.success('路径已复制到剪贴板')
  }
  catch (err) {
    message.error('复制失败')
  }
}

// 切换项目监听状态
async function toggleWatching(projectRoot: string, currentlyWatching: boolean) {
  try {
    if (currentlyWatching) {
      await invoke('stop_project_watching', { projectRootPath: projectRoot })
      message.success('已停止监听项目')
    }
    else {
      // 开启监听（后端会自动添加到监听列表）
      await invoke('trigger_acemcp_index_update', { projectRootPath: projectRoot })
      message.success('已开启监听项目')
    }
    // 重新加载监听列表
    await fetchWatchingProjects()
    watchingProjects.value = await invoke<string[]>('get_watching_projects')
  }
  catch (err) {
    console.error('切换监听状态失败:', err)
    message.error('操作失败')
  }
}

// 重新索引（带二次确认）
function handleReindex(projectRoot: string) {
  dialog.warning({
    title: '确认重新索引',
    content: `确定要重新索引项目吗？\n\n${projectRoot}\n\n这将重新扫描所有文件并更新索引。`,
    positiveText: '确认',
    negativeText: '取消',
    onPositiveClick: async () => {
      try {
        await triggerIndexUpdate(projectRoot)
        message.success('已触发重新索引')
        // 延迟刷新状态
        setTimeout(() => loadAllData(), 1000)
      }
      catch (err) {
        console.error('重新索引失败:', err)
        message.error('重新索引失败')
      }
    },
  })
}

// 查看项目结构树
function viewProjectTree(projectRoot: string) {
  selectedProject.value = projectRoot
  showDrawer.value = true
}

// 抽屉中的重新同步处理
async function handleDrawerResync() {
  if (!selectedProject.value)
    return
  resyncLoading.value = true
  try {
    await triggerIndexUpdate(selectedProject.value)
    message.success('已触发重新索引')
    // 延迟刷新状态
    setTimeout(() => loadAllData(), 1000)
  }
  catch (err) {
    console.error('重新索引失败:', err)
    message.error('重新索引失败')
  }
  finally {
    resyncLoading.value = false
  }
}

// 格式化时间
function formatTime(timeStr: string | null): string {
  if (!timeStr)
    return '从未索引'
  try {
    const date = new Date(timeStr)
    return date.toLocaleString('zh-CN', {
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    })
  }
  catch {
    return '时间格式错误'
  }
}

// 获取状态配置
function getStatusConfig(status: string) {
  const configs = {
    idle: {
      text: '未索引',
      type: 'default' as const,
      color: 'text-gray-500 dark:text-gray-400',
      bgColor: 'bg-gray-100 dark:bg-gray-800',
      icon: 'i-carbon-circle-dash',
    },
    indexing: {
      text: '索引中',
      type: 'info' as const,
      color: 'text-blue-500 dark:text-blue-400',
      bgColor: 'bg-blue-100 dark:bg-blue-900',
      icon: 'i-carbon-in-progress animate-spin',
    },
    synced: {
      text: '已完成',
      type: 'success' as const,
      color: 'text-green-500 dark:text-green-400',
      bgColor: 'bg-green-100 dark:bg-green-900',
      icon: 'i-carbon-checkmark-filled',
    },
    failed: {
      text: '失败',
      type: 'error' as const,
      color: 'text-red-500 dark:text-red-400',
      bgColor: 'bg-red-100 dark:bg-red-900',
      icon: 'i-carbon-warning-filled',
    },
  }
  return configs[status as keyof typeof configs] || configs.idle
}
</script>

<template>
  <div class="project-index-manager">
    <!-- 加载状态 -->
    <div v-if="loading" class="space-y-4">
      <n-skeleton v-for="i in 3" :key="i" text :repeat="3" class="mb-4" />
    </div>

    <!-- 空状态 -->
    <n-empty
      v-else-if="projectList.length === 0"
      description="暂无项目索引数据"
      class="py-8"
    >
      <template #icon>
        <div class="i-carbon-folder-off text-4xl opacity-40" />
      </template>
      <template #extra>
        <div class="text-sm opacity-60 mt-2">
          打开项目后将自动显示索引状态
        </div>
      </template>
    </n-empty>

    <!-- 项目列表 -->
    <div v-else class="space-y-4">
      <div
        v-for="project in projectList"
        :key="project.project_root"
        class="project-card"
        :class="getStatusConfig(project.status).bgColor"
      >
        <div class="flex flex-col gap-3 p-4">
          <!-- 顶部：项目路径和状态 -->
          <div class="flex items-start justify-between gap-3">
            <!-- 项目路径 -->
            <n-tooltip trigger="hover">
              <template #trigger>
                <div
                  class="flex-1 font-mono text-sm cursor-pointer hover:text-primary-600 dark:hover:text-primary-400 transition-colors truncate"
                  @click="copyPath(project.project_root)"
                >
                  <div class="i-carbon-folder inline-block mr-2 opacity-60" />
                  {{ project.project_root }}
                </div>
              </template>
              点击复制路径
            </n-tooltip>

            <!-- 状态徽章 -->
            <n-tag
              :type="getStatusConfig(project.status).type"
              :bordered="false"
              size="small"
              class="flex-shrink-0"
            >
              <template #icon>
                <div :class="[getStatusConfig(project.status).icon, 'text-sm']" />
              </template>
              {{ getStatusConfig(project.status).text }}
            </n-tag>
          </div>

          <!-- 进度条（仅索引中时显示） -->
          <div v-if="project.status === 'indexing'" class="w-full">
            <n-progress
              type="line"
              :percentage="project.progress"
              :show-indicator="true"
              :height="8"
              :border-radius="4"
              processing
            />
          </div>

          <!-- 统计信息 -->
          <div class="flex items-center gap-4 text-xs opacity-70 flex-wrap">
            <n-tooltip trigger="hover">
              <template #trigger>
                <div class="flex items-center gap-1">
                  <div class="i-carbon-document" />
                  <span>总计: {{ project.total_files }}</span>
                </div>
              </template>
              项目中的总文件数
            </n-tooltip>
            <n-tooltip trigger="hover">
              <template #trigger>
                <div class="flex items-center gap-1">
                  <div class="i-carbon-checkmark-filled text-green-500" />
                  <span>已索引: {{ project.indexed_files }}</span>
                </div>
              </template>
              已成功索引的文件数
            </n-tooltip>
            <n-tooltip v-if="project.pending_files > 0" trigger="hover">
              <template #trigger>
                <div class="flex items-center gap-1">
                  <div class="i-carbon-time text-blue-500" />
                  <span>待处理: {{ project.pending_files }}</span>
                </div>
              </template>
              等待索引的文件数
            </n-tooltip>
            <n-tooltip v-if="project.failed_files > 0" trigger="hover">
              <template #trigger>
                <div class="flex items-center gap-1">
                  <div class="i-carbon-warning-filled text-red-500" />
                  <span>失败: {{ project.failed_files }}</span>
                </div>
              </template>
              索引失败的文件数
            </n-tooltip>
          </div>

          <!-- 索引时间 -->
          <div class="text-xs opacity-60">
            <n-tooltip trigger="hover">
              <template #trigger>
                <div class="flex items-center gap-1">
                  <div class="i-carbon-time" />
                  <span>最后索引: {{ formatTime(project.last_success_time) }}</span>
                </div>
              </template>
              {{ project.last_success_time ? '上次成功索引的时间' : '该项目尚未成功索引' }}
            </n-tooltip>
          </div>

          <!-- 操作按钮组 -->
          <div class="flex items-center gap-2 pt-2 border-t border-gray-200 dark:border-gray-700">
            <!-- 监听开关 -->
            <n-tooltip trigger="hover">
              <template #trigger>
                <div class="flex items-center gap-2">
                  <n-switch
                    :value="watchingProjects.includes(project.project_root)"
                    size="small"
                    @update:value="toggleWatching(project.project_root, watchingProjects.includes(project.project_root))"
                  >
                    <template #checked>
                      <div class="i-carbon-view text-xs" />
                    </template>
                    <template #unchecked>
                      <div class="i-carbon-view-off text-xs" />
                    </template>
                  </n-switch>
                  <span class="text-xs opacity-70">实时监听</span>
                </div>
              </template>
              实时同步上传项目变更
            </n-tooltip>

            <div class="flex-1" />

            <!-- 重新索引按钮 -->
            <n-button
              size="small"
              secondary
              type="primary"
              :disabled="project.status === 'indexing'"
              @click="handleReindex(project.project_root)"
            >
              <template #icon>
                <div class="i-carbon-renew" />
              </template>
              重新索引
            </n-button>

            <!-- 查看结构树按钮 -->
            <n-button
              size="small"
              secondary
              type="info"
              @click="viewProjectTree(project.project_root)"
            >
              <template #icon>
                <div class="i-carbon-tree-view" />
              </template>
              查看结构树
            </n-button>
          </div>
        </div>
      </div>
    </div>

    <!-- 项目结构树抽屉 -->
    <McpIndexStatusDrawer
      v-model:show="showDrawer"
      :project-root="selectedProject"
      :status-summary="selectedStatusSummary"
      :status-icon="selectedStatusIcon"
      :project-status="selectedProjectStatus"
      :is-indexing="selectedIsIndexing"
      :resync-loading="resyncLoading"
      @resync="handleDrawerResync"
    />
  </div>
</template>

<style scoped>
/* 项目索引管理容器 */
.project-index-manager {
  max-width: 900px;
  margin: 0 auto;
}

/* 项目卡片样式 - 科技感设计 */
.project-card {
  position: relative;
  border-radius: 12px;
  overflow: hidden;
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  border: 1px solid rgba(128, 128, 128, 0.2);
  backdrop-filter: blur(8px);
}

/* 卡片悬停效果 */
.project-card:hover {
  transform: translateY(-2px);
  box-shadow:
    0 8px 25px -5px rgba(0, 0, 0, 0.1),
    0 10px 10px -5px rgba(0, 0, 0, 0.04);
  border-color: rgba(59, 130, 246, 0.3);
}

/* 科技感光效扫描动画 */
.project-card::before {
  content: '';
  position: absolute;
  top: 0;
  left: -100%;
  width: 100%;
  height: 100%;
  background: linear-gradient(
    90deg,
    transparent,
    rgba(255, 255, 255, 0.08),
    transparent
  );
  transition: left 0.6s ease;
  pointer-events: none;
}

.project-card:hover::before {
  left: 100%;
}

/* 深色模式下的光效调整 */
:deep(.dark) .project-card::before {
  background: linear-gradient(
    90deg,
    transparent,
    rgba(255, 255, 255, 0.05),
    transparent
  );
}

/* 卡片顶部装饰线 - 科技感 */
.project-card::after {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  height: 2px;
  background: linear-gradient(
    90deg,
    transparent,
    rgba(59, 130, 246, 0.5),
    rgba(147, 51, 234, 0.5),
    transparent
  );
  opacity: 0;
  transition: opacity 0.3s ease;
}

.project-card:hover::after {
  opacity: 1;
}
</style>
