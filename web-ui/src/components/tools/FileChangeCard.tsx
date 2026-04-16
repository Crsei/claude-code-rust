import { FileText, FilePlus, FilePen } from 'lucide-react'

interface FileChangeCardProps {
  content: string
  toolName: string
  input?: Record<string, unknown>
}

/**
 * Display file operation results with path and content.
 * For write/edit operations, shows the file path and result content.
 */
export function FileChangeCard({ content, toolName, input }: FileChangeCardProps) {
  const filePath = typeof input?.file_path === 'string'
    ? input.file_path
    : typeof input?.path === 'string'
      ? input.path
      : null

  const isWrite = toolName.toLowerCase().includes('write')
  const isEdit = toolName.toLowerCase().includes('edit')

  const Icon = isWrite ? FilePlus : isEdit ? FilePen : FileText

  return (
    <div className="rounded border border-border/40 bg-muted/30 overflow-hidden">
      {/* File header */}
      {filePath && (
        <div className="flex items-center gap-2 border-b border-border/40 bg-muted/50 px-3 py-1.5">
          <Icon className="h-3.5 w-3.5 text-muted-foreground" />
          <span className="text-[11px] font-mono text-foreground/80 truncate">{filePath}</span>
          <span className="ml-auto text-[10px] text-muted-foreground">
            {isWrite ? 'written' : isEdit ? 'edited' : 'file op'}
          </span>
        </div>
      )}

      {/* Content */}
      {content && (
        <pre className="overflow-x-auto px-3 py-2 text-[11px] font-mono text-foreground/80 max-h-60 overflow-y-auto whitespace-pre-wrap">
          {content}
        </pre>
      )}
      {!content && (
        <div className="px-3 py-2 text-[11px] text-muted-foreground italic">
          Operation completed successfully
        </div>
      )}
    </div>
  )
}
