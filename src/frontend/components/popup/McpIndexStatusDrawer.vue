<script setup lang="ts">
import type { FileIndexStatusType, ProjectFilesStatus, ProjectIndexStatus } from '../../types/tauri'
import type { TreeOption } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'
import { useMessage } from 'naive-ui'
import { computed, h, ref, watch } from 'vue'

interface Props {
  show: boolean
  projectRoot: string
  statusSummary: string
  statusIcon: string
  projectStatus: ProjectIndexStatus | null
  isIndexing?: boolean
  resyncLoading?: boolean
}

interface Emits {
  'update:show': [value: boolean]
  'resync': []
}

const props = withDefaults(defineProps<Props>(), {
  isIndexing: false,
  resyncLoading: false,
})

const emit = defineEmits<Emits>()

// 抽屉显示状态，使用 v-model:show 双向绑定父组件
const drawerVisible = computed({
  get: () => props.show,
  set: (val: boolean) => emit('update:show', val),
})

// 规范化展示路径（去掉 Windows 扩展前缀并统一斜杠）
const displayPath = computed(() => {
  let p = props.projectRoot || ''
  // 处理 Windows 扩展路径前缀 \\?\ 或 //?/
  if (p.startsWith('\\\\?\\'))
    p = p.slice(4)
  else if (p.startsWith('//?/'))
    p = p.slice(4)
  // 统一使用正斜杠
  return p.replace(/\\/g, '/')
})

// 文件索引状态数据
const filesStatus = ref<ProjectFilesStatus | null>(null)
const loadingFiles = ref(false)
const filesError = ref<string | null>(null)
// 是否仅显示未完全同步的文件（过滤开关）
const showOnlyPending = ref(false)

const message = useMessage()

// Tree 节点类型
type NodeStatus = 'indexed' | 'pending'

// 扩展的树节点接口，包含图标渲染所需的额外信息
interface IndexTreeNode {
  key: string
  label: string
  children?: IndexTreeNode[]
  // 仅文件节点使用的状态
  status?: NodeStatus
  // 是否为目录节点
  isDirectory?: boolean
  // 文件扩展名（用于图标映射）
  fileExtension?: string
  // 原始文件名（不含状态后缀）
  fileName?: string
}

// ==================== 文件图标映射系统 ====================

// 文件图标配置接口
interface FileIconConfig {
  icon: string
  color: string
}

// 文件扩展名到图标的映射表
const FILE_ICON_MAP: Record<string, FileIconConfig> = {
  // Rust
  rs: { icon: 'i-carbon-code', color: '#dea584' },
  // Vue
  vue: { icon: 'i-carbon-application', color: '#42b883' },
  // TypeScript
  ts: { icon: 'i-carbon-code', color: '#3178c6' },
  tsx: { icon: 'i-carbon-code', color: '#3178c6' },
  // JavaScript
  js: { icon: 'i-carbon-code', color: '#f7df1e' },
  jsx: { icon: 'i-carbon-code', color: '#f7df1e' },
  // Python
  py: { icon: 'i-carbon-code', color: '#3776ab' },
  // JSON
  json: { icon: 'i-carbon-json', color: '#cbcb41' },
  // Markdown
  md: { icon: 'i-carbon-document', color: '#519aba' },
  // HTML
  html: { icon: 'i-carbon-html', color: '#e34c26' },
  htm: { icon: 'i-carbon-html', color: '#e34c26' },
  // CSS
  css: { icon: 'i-carbon-css', color: '#264de4' },
  scss: { icon: 'i-carbon-css', color: '#c6538c' },
  sass: { icon: 'i-carbon-css', color: '#c6538c' },
  less: { icon: 'i-carbon-css', color: '#1d365d' },
  // YAML/TOML
  yaml: { icon: 'i-carbon-document', color: '#cb171e' },
  yml: { icon: 'i-carbon-document', color: '#cb171e' },
  toml: { icon: 'i-carbon-document', color: '#9c4121' },
  // XML
  xml: { icon: 'i-carbon-document', color: '#e37933' },
  // SQL
  sql: { icon: 'i-carbon-data-base', color: '#336791' },
  // Shell
  sh: { icon: 'i-carbon-terminal', color: '#89e051' },
  bash: { icon: 'i-carbon-terminal', color: '#89e051' },
  // Go
  go: { icon: 'i-carbon-code', color: '#00add8' },
  // Java
  java: { icon: 'i-carbon-code', color: '#b07219' },
  // C/C++
  c: { icon: 'i-carbon-code', color: '#555555' },
  cpp: { icon: 'i-carbon-code', color: '#f34b7d' },
  h: { icon: 'i-carbon-code', color: '#555555' },
  hpp: { icon: 'i-carbon-code', color: '#f34b7d' },
  // C#
  cs: { icon: 'i-carbon-code', color: '#178600' },
  // Ruby
  rb: { icon: 'i-carbon-code', color: '#701516' },
  // PHP
  php: { icon: 'i-carbon-code', color: '#4f5d95' },
  // 文本文件
  txt: { icon: 'i-carbon-document-blank', color: '#6b7280' },
}

// 默认文件图标
const DEFAULT_FILE_ICON: FileIconConfig = {
  icon: 'i-carbon-document-blank',
  color: '#6b7280',
}

// 目录图标配置
const DIRECTORY_ICON: FileIconConfig = {
  icon: 'i-carbon-folder',
  color: '#14b8a6',
}

// 获取文件图标配置
function getFileIconConfig(fileName: string, isDirectory: boolean): FileIconConfig {
  if (isDirectory) {
    return DIRECTORY_ICON
  }
  const ext = fileName.split('.').pop()?.toLowerCase() || ''
  return FILE_ICON_MAP[ext] || DEFAULT_FILE_ICON
}

// 根据后端返回的文件列表构建简单树结构
const treeData = computed<IndexTreeNode[]>(() => {
  const result: IndexTreeNode[] = []
  const allFiles = filesStatus.value?.files ?? []

  // 根据开关过滤文件列表：仅保留状态不是 indexed 的文件
  const files = showOnlyPending.value
    ? allFiles.filter(file => file.status !== 'indexed')
    : allFiles

  for (const file of files) {
    insertPath(result, file.path, file.status)
  }

  // 构建完成后，为目录节点计算聚合状态并更新标签文案
  aggregateDirectoryStats(result)

  return result
})

// 将单个文件路径插入到树结构中
function insertPath(nodes: IndexTreeNode[], path: string, status: FileIndexStatusType) {
  // 只区分 indexed / pending 两种状态，mixed 由前端文案解释
  const normalizedStatus: NodeStatus = status === 'indexed' ? 'indexed' : 'pending'

  const segments = path.split('/').filter(Boolean)
  let current = nodes
  let currentPath = ''

  segments.forEach((segment, index) => {
    currentPath = currentPath ? `${currentPath}/${segment}` : segment
    let node = current.find(n => n.key === currentPath)

    const isLeaf = index === segments.length - 1

    if (!node) {
      // 提取文件扩展名
      const ext = segment.includes('.') ? segment.split('.').pop()?.toLowerCase() : undefined

      node = {
        key: currentPath,
        label: segment,
        fileName: segment,
        isDirectory: !isLeaf,
        fileExtension: isLeaf ? ext : undefined,
      }
      current.push(node)
    }

    if (isLeaf) {
      // 文件节点：保存原始文件名和状态
      node.status = normalizedStatus
      node.isDirectory = false
    }
    else {
      // 目录节点
      node.isDirectory = true
      if (!node.children)
        node.children = []
      current = node.children
    }
  })
}

// 计算目录节点的聚合状态，并更新目录标签（显示已索引/总文件数）
function aggregateDirectoryStats(nodes: IndexTreeNode[]) {
  nodes.forEach((node) => {
    aggregateNode(node)
  })
}

function aggregateNode(node: IndexTreeNode): { total: number, indexed: number } {
  if (!node.children || node.children.length === 0) {
    const total = node.status ? 1 : 0
    const indexed = node.status === 'indexed' ? 1 : 0
    return { total, indexed }
  }

  let total = 0
  let indexed = 0

  for (const child of node.children) {
    const childAgg = aggregateNode(child)
    total += childAgg.total
    indexed += childAgg.indexed
  }

  if (total > 0) {
    const baseLabel = node.label.split(' · ')[0]
    let suffix: string

    if (indexed === 0) {
      suffix = '未索引'
    }
    else if (indexed === total) {
      suffix = `全部已索引 (${indexed})`
    }
    else {
      suffix = `${indexed}/${total} 已索引`
    }

    node.label = `${baseLabel} · ${suffix}`
  }

  return { total, indexed }
}

// 加载指定项目的文件索引状态
async function fetchFilesStatus() {
  if (!props.projectRoot)
    return

  loadingFiles.value = true
  filesError.value = null

  try {
    // 调用 Tauri 命令获取文件级索引状态
    const result = await invoke<ProjectFilesStatus>('get_acemcp_project_files_status', {
      projectRootPath: props.projectRoot,
    })
    filesStatus.value = result
  }
  catch (err) {
    console.error('获取项目文件索引状态失败:', err)
    filesError.value = String(err)
    message.error('加载项目结构失败，请检查索引配置')
  }
  finally {
    loadingFiles.value = false
  }
}

// 当抽屉打开且有项目路径时，自动加载一次文件状态
watch(
  () => props.show,
  (visible) => {
    if (visible && props.projectRoot)
      fetchFilesStatus()
  },
)

// 手动重新同步按钮点击
function handleResyncClick() {
  emit('resync')
}

// ==================== 自定义树节点渲染 ====================

// 渲染节点前缀图标
function renderPrefix({ option }: { option: TreeOption }) {
  const node = option as unknown as IndexTreeNode
  const iconConfig = getFileIconConfig(node.fileName || node.label, node.isDirectory || false)

  return h('div', {
    class: `${iconConfig.icon} w-4 h-4 flex-shrink-0 transition-colors duration-200`,
    style: { color: iconConfig.color },
  })
}

// 渲染节点标签
function renderLabel({ option }: { option: TreeOption }) {
  const node = option as unknown as IndexTreeNode
  const isDirectory = node.isDirectory || false
  const fileName = node.fileName || node.label.split(' · ')[0]

  // 目录节点：显示目录名和统计信息
  if (isDirectory) {
    const stats = node.label.includes(' · ') ? node.label.split(' · ')[1] : ''
    return h('div', { class: 'flex items-center gap-2 py-0.5' }, [
      h('span', { class: 'text-white font-medium truncate' }, fileName),
      stats
        ? h('span', {
            class: `text-[10px] px-1.5 py-0.5 rounded-full ${
              stats.includes('全部已索引')
                ? 'bg-green-500/20 text-green-400'
                : stats.includes('未索引')
                  ? 'bg-orange-500/20 text-orange-400'
                  : 'bg-blue-500/20 text-blue-400'
            }`,
          }, stats)
        : null,
    ])
  }

  // 文件节点：显示文件名和状态标签
  const status = node.status
  return h('div', { class: 'flex items-center gap-2 py-0.5' }, [
    h('span', { class: 'text-gray-300 truncate' }, fileName),
    h('span', {
      class: `text-[10px] px-1.5 py-0.5 rounded-full ${
        status === 'indexed'
          ? 'bg-green-500/20 text-green-400'
          : 'bg-orange-500/20 text-orange-400'
      }`,
    }, status === 'indexed' ? '已索引' : '未同步'),
  ])
}
</script>

<template>
  <n-drawer
    v-model:show="drawerVisible"
    placement="right"
    :width="420"
    :trap-focus="false"
  >
    <n-drawer-content>
      <template #header>
        <div class="flex items-center gap-2">
          <div :class="statusIcon" class="w-4 h-4" />
          <span class="text-sm font-medium">代码索引状态</span>
        </div>
      </template>

      <div class="space-y-4 text-xs">
        <!-- 项目基础信息 -->
        <div class="space-y-1">
          <div class="flex items-center justify-between">
            <span class="text-gray-500">项目路径</span>
            <span class="ml-2 truncate max-w-[260px]" :title="displayPath">
              {{ displayPath || '未提供' }}
            </span>
          </div>
          <div class="flex items-center justify-between">
            <span class="text-gray-500">整体状态</span>
            <span class="ml-2 font-medium">
              {{ statusSummary }}
            </span>
          </div>
          <div v-if="projectStatus" class="flex items-center justify-between">
            <span class="text-gray-500">进度</span>
            <span class="ml-2">
              {{ projectStatus.progress }}%
              <span class="ml-1 text-gray-500">
                ({{ projectStatus.indexed_files }}/{{ projectStatus.total_files }})
              </span>
            </span>
          </div>
        </div>

        <!-- 总体进度条 -->
        <div v-if="projectStatus">
          <n-progress
            type="line"
            :percentage="projectStatus.progress"
            :height="6"
            :border-radius="3"
            :show-indicator="false"
            :status="projectStatus.status === 'failed' ? 'error' : 'info'"
          />
        </div>

        <!-- 项目结构树 -->
        <div class="space-y-2">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-3">
              <span class="text-xs font-medium text-gray-500">项目结构</span>
              <!-- 仅显示未完全同步文件开关 -->
              <div class="flex items-center gap-1 text-[11px] text-gray-500">
                <n-switch
                  v-model:value="showOnlyPending"
                  size="small"
                />
                <span>仅显示未完全同步</span>
              </div>
            </div>
            <n-button
              text
              size="tiny"
              :loading="loadingFiles"
              @click="fetchFilesStatus"
            >
              <template #icon>
                <div class="i-carbon-renew w-3 h-3" />
              </template>
              刷新
            </n-button>
          </div>

          <div class="tree-container min-h-[120px] max-h-[280px] overflow-y-auto pr-1">
            <!-- 骨架屏加载状态 -->
            <div v-if="loadingFiles" class="tree-skeleton space-y-2 py-2">
              <div v-for="i in 5" :key="i" class="skeleton-item flex items-center gap-2" :style="{ paddingLeft: `${(i % 3) * 16}px` }">
                <div class="skeleton-icon w-4 h-4 rounded" />
                <div class="skeleton-text h-4 rounded" :style="{ width: `${60 + Math.random() * 80}px` }" />
                <div v-if="i % 2 === 0" class="skeleton-badge h-4 w-12 rounded-full" />
              </div>
            </div>

            <!-- 错误状态 -->
            <div v-else-if="filesError" class="flex items-center gap-2 text-red-400 py-3 px-2 bg-red-500/10 rounded-lg">
              <div class="i-carbon-warning-alt w-4 h-4 flex-shrink-0" />
              <span class="text-xs">{{ filesError }}</span>
            </div>

            <!-- 空状态 -->
            <div v-else-if="!treeData.length" class="flex flex-col items-center justify-center py-6 text-gray-500">
              <div class="i-carbon-folder-off w-8 h-8 mb-2 opacity-50" />
              <span class="text-xs text-center">暂无可索引文件<br>请确认扩展名和排除规则配置</span>
            </div>

            <!-- 项目结构树 -->
            <div v-else class="custom-tree">
              <n-tree
                :data="treeData"
                :block-line="true"
                :selectable="false"
                :expand-on-click="true"
                :render-prefix="renderPrefix"
                :render-label="renderLabel"
                :default-expand-all="false"
                :animated="true"
              />
            </div>
          </div>
        </div>

        <!-- 手动重新同步控制 -->
        <div class="pt-2 border-t border-gray-200 flex items-center justify-between gap-3">
          <div class="text-[11px] text-gray-500 leading-snug space-y-0.5">
            <div>重新同步会在后台执行，不会阻塞当前对话。</div>
            <div v-if="projectStatus?.last_success_time">
              上次成功：{{ projectStatus.last_success_time }}
            </div>
            <div v-if="projectStatus?.failed_files">
              失败文件数：<span class="text-red-500">{{ projectStatus.failed_files }}</span>
            </div>
            <div v-if="projectStatus?.last_error" class="text-red-500">
              最近错误：{{ projectStatus.last_error }}
            </div>
          </div>
          <n-button
            type="primary"
            size="small"
            :loading="resyncLoading || isIndexing"
            :disabled="resyncLoading || isIndexing || !projectRoot"
            strong
            @click="handleResyncClick"
          >
            <template #icon>
              <div class="i-carbon-renew w-4 h-4" />
            </template>
            {{ isIndexing ? '索引中...' : '重新同步' }}
          </n-button>
        </div>
      </div>
    </n-drawer-content>
  </n-drawer>
</template>

<style scoped>
/* ==================== 树形容器样式 ==================== */
.tree-container {
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.2);
  padding: 8px;
  border: 1px solid var(--color-border, rgba(255, 255, 255, 0.1));
}

/* ==================== 骨架屏样式 ==================== */
.tree-skeleton .skeleton-item {
  animation: skeleton-fade 1.5s ease-in-out infinite;
}

.tree-skeleton .skeleton-item:nth-child(2) { animation-delay: 0.1s; }
.tree-skeleton .skeleton-item:nth-child(3) { animation-delay: 0.2s; }
.tree-skeleton .skeleton-item:nth-child(4) { animation-delay: 0.3s; }
.tree-skeleton .skeleton-item:nth-child(5) { animation-delay: 0.4s; }

.skeleton-icon {
  background: linear-gradient(90deg, rgba(255, 255, 255, 0.05) 25%, rgba(255, 255, 255, 0.1) 50%, rgba(255, 255, 255, 0.05) 75%);
  background-size: 200% 100%;
  animation: skeleton-shimmer 1.5s ease-in-out infinite;
}

.skeleton-text {
  background: linear-gradient(90deg, rgba(255, 255, 255, 0.08) 25%, rgba(255, 255, 255, 0.15) 50%, rgba(255, 255, 255, 0.08) 75%);
  background-size: 200% 100%;
  animation: skeleton-shimmer 1.5s ease-in-out infinite;
}

.skeleton-badge {
  background: linear-gradient(90deg, rgba(255, 255, 255, 0.05) 25%, rgba(255, 255, 255, 0.1) 50%, rgba(255, 255, 255, 0.05) 75%);
  background-size: 200% 100%;
  animation: skeleton-shimmer 1.5s ease-in-out infinite;
}

@keyframes skeleton-shimmer {
  0% { background-position: 200% 0; }
  100% { background-position: -200% 0; }
}

@keyframes skeleton-fade {
  0%, 100% { opacity: 0.6; }
  50% { opacity: 1; }
}

/* ==================== 自定义树样式 ==================== */
.custom-tree :deep(.n-tree) {
  --n-node-text-color: var(--color-on-surface, #e5e7eb);
  --n-node-text-color-disabled: var(--color-on-surface-muted, #9ca3af);
}

/* 树节点基础样式 */
.custom-tree :deep(.n-tree-node) {
  border-radius: 6px;
  margin-bottom: 2px;
  transition: all 0.2s ease;
}

/* 树节点悬停效果 */
.custom-tree :deep(.n-tree-node:hover) {
  background: rgba(255, 255, 255, 0.05);
}

/* 树节点内容区域 */
.custom-tree :deep(.n-tree-node-content) {
  padding: 4px 8px;
  border-radius: 6px;
}

/* 展开/折叠图标样式 */
.custom-tree :deep(.n-tree-node-switcher) {
  width: 20px;
  height: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: transform 0.2s ease;
}

/* 展开状态的图标旋转 */
.custom-tree :deep(.n-tree-node-switcher--expanded) {
  transform: rotate(90deg);
}

/* 树形引导线 - IntelliJ IDEA 风格 */
.custom-tree :deep(.n-tree-node-indent) {
  position: relative;
}

.custom-tree :deep(.n-tree-node-indent::before) {
  content: '';
  position: absolute;
  left: 10px;
  top: 0;
  bottom: 0;
  width: 1px;
  background: linear-gradient(
    to bottom,
    transparent 0%,
    rgba(255, 255, 255, 0.1) 10%,
    rgba(255, 255, 255, 0.1) 90%,
    transparent 100%
  );
}

/* 节点前缀图标容器 */
.custom-tree :deep(.n-tree-node-content__prefix) {
  margin-right: 8px;
  display: flex;
  align-items: center;
}

/* 节点标签容器 */
.custom-tree :deep(.n-tree-node-content__text) {
  flex: 1;
  min-width: 0;
  overflow: hidden;
}

/* 选中状态样式（虽然禁用了选择，但保留样式以备后用） */
.custom-tree :deep(.n-tree-node--selected) {
  background: rgba(20, 184, 166, 0.15);
  border-left: 2px solid #14b8a6;
}

/* 展开动画 */
.custom-tree :deep(.n-tree-node-children) {
  overflow: hidden;
}

/* 浅色主题适配 */
:root.light .tree-container {
  background: rgba(0, 0, 0, 0.03);
  border-color: rgba(0, 0, 0, 0.1);
}

:root.light .skeleton-icon,
:root.light .skeleton-text,
:root.light .skeleton-badge {
  background: linear-gradient(90deg, rgba(0, 0, 0, 0.05) 25%, rgba(0, 0, 0, 0.1) 50%, rgba(0, 0, 0, 0.05) 75%);
  background-size: 200% 100%;
}

:root.light .custom-tree :deep(.n-tree-node:hover) {
  background: rgba(0, 0, 0, 0.03);
}

:root.light .custom-tree :deep(.n-tree-node-indent::before) {
  background: linear-gradient(
    to bottom,
    transparent 0%,
    rgba(0, 0, 0, 0.1) 10%,
    rgba(0, 0, 0, 0.1) 90%,
    transparent 100%
  );
}
</style>
