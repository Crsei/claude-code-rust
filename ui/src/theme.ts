/** Hex color palette — maps old ansi:xxx names to concrete values */
export const c = {
  accent:      '#CC00CC',  // magenta
  accentBright:'#FF55FF',  // magentaBright
  user:        '#55FFFF',  // cyanBright
  userBubbleBg:'#08131A',
  userBubbleBorder:'#16798A',
  toolQuestionBg:'#171200',
  toolQuestionBorder:'#8A6B16',
  success:     '#4EC940',  // green
  successBright:'#55FF55', // greenBright
  error:       '#CC0000',  // red
  errorBright: '#FF5555',  // redBright
  warning:     '#C4A500',  // yellow
  warningBright:'#FFFF55', // yellowBright
  info:        '#00AAAA',  // cyan
  infoBright:  '#55FFFF',  // cyanBright
  blue:        '#3D6DCC',  // blue
  dim:         '#888888',  // blackBright equivalent
  muted:       '#666666',
  text:        '#CCCCCC',  // default foreground
  textBright:  '#FFFFFF',
  bg:          '#000000',
} as const

/** Legacy theme object — kept for reference during migration */
export const theme = {
  assistantName: { color: c.accent,  bold: true },
  userName:      { color: c.user,    bold: true },
  systemName:    { color: c.text,    dim: true },
  toolName:      { color: c.warning, bold: true },
  error:         { color: c.error,   bold: true },
  warning:       { color: c.warning },
  info:          { color: c.info },
  code:          { color: c.warning },
  diffAdd:       { color: c.success },
  diffRemove:    { color: c.error },
  diffMeta:      { color: c.info, dim: true },
  border:        { color: c.text, dim: true },
  accent:        { color: c.accent },
  muted:         { dim: true },
} as const
