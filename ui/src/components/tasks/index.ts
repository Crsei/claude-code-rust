/**
 * Tasks panel barrel export.
 *
 * Adapted subset of the upstream `components/tasks/` folder. Files not
 * ported (because they require backend task/agent-detail subsystems we
 * don't have in the Rust port):
 *   - AsyncAgentDetailDialog, DreamDetailDialog, InProcessTeammateDetailDialog,
 *     MonitorMcpDetailDialog, RemoteSessionDetailDialog, RemoteSessionProgress,
 *     ShellDetailDialog, WorkflowDetailDialog, BackgroundTasksDialog
 */
export { BackgroundTask } from './BackgroundTask.js'
export { BackgroundTaskStatus } from './BackgroundTaskStatus.js'
export {
  ShellProgress,
  TaskStatusText,
  bashCommandFromActivity,
  toolStatusToTaskStatus,
} from './ShellProgress.js'
export { renderToolActivityChip } from './renderToolActivity.js'
export {
  elapsedMs,
  formatElapsed,
  getTaskStatusColor,
  getTaskStatusIcon,
  isTerminalStatus,
  statusOf,
  type TaskStatus,
  type TaskStatusOptions,
} from './taskStatusUtils.js'
