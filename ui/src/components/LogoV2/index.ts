/**
 * Barrel for the LogoV2 welcome-screen surface. Mirrors the upstream
 * layout at `ui/examples/upstream-patterns/src/components/LogoV2/` so
 * downstream imports stay stable during the full-build migration.
 */
export { AnimatedAsterisk } from './AnimatedAsterisk.js'
export { AnimatedClawd, CLAWD_ANIMATIONS } from './AnimatedClawd.js'
export { ChannelsNotice } from './ChannelsNotice.js'
export type {
  ChannelEntryDisplay,
  ChannelsNoticeStatus,
  ChannelsNoticeUnmatched,
} from './ChannelsNotice.js'
export { Clawd, CLAWD_COLORS } from './Clawd.js'
export type { ClawdPose } from './Clawd.js'
export { CondensedLogo } from './CondensedLogo.js'
export type { CondensedLogoData } from './CondensedLogo.js'
export { EmergencyTip } from './EmergencyTip.js'
export type { EmergencyTipColor } from './EmergencyTip.js'
export { ExperimentEnrollmentNotice } from './ExperimentEnrollmentNotice.js'
export { Feed, calculateFeedWidth } from './Feed.js'
export type { FeedConfig, FeedLine } from './Feed.js'
export { FeedColumn } from './FeedColumn.js'
export {
  createGuestPassesFeed,
  createOverageCreditFeed,
  createProjectOnboardingFeed,
  createRecentActivityFeed,
  createWhatsNewFeed,
} from './feedConfigs.js'
export type { OnboardingStep, RecentActivity } from './feedConfigs.js'
export { GateOverridesWarning } from './GateOverridesWarning.js'
export { GuestPassesUpsell } from './GuestPassesUpsell.js'
export { LogoV2 } from './LogoV2.js'
export type { LogoV2LayoutMode, LogoV2ViewData } from './LogoV2.js'
export { Opus1mMergeNotice, shouldShowOpus1mMergeNotice } from './Opus1mMergeNotice.js'
export { OverageCreditUpsell } from './OverageCreditUpsell.js'
export { VoiceModeNotice } from './VoiceModeNotice.js'
export { WelcomeV2 } from './WelcomeV2.js'
