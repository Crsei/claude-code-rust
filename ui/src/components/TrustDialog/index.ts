/**
 * Barrel for the TrustDialog folder, mirroring upstream
 * `ui/examples/upstream-patterns/src/components/TrustDialog/` which
 * exports the dialog plus the setting-source helpers.
 */
export { TrustDialog, type TrustDialogSignals } from './TrustDialog.js'
export {
  formatListWithAnd,
  getApiKeyHelperSources,
  getAwsCommandsSources,
  getBashPermissionSources,
  getDangerousEnvVarsSources,
  getGcpCommandsSources,
  getHooksSources,
  getOtelHeadersHelperSources,
  type BashRule,
  type PermissionSnapshot,
  type SettingsSnapshot,
} from './utils.js'
