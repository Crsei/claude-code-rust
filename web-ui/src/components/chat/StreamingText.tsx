interface StreamingTextProps {
  content: string
}

export function StreamingText({ content }: StreamingTextProps) {
  return (
    <div className="flex justify-start">
      <div className="max-w-[85%] rounded-2xl rounded-bl-md bg-secondary px-4 py-2.5 text-sm text-secondary-foreground">
        <div className="whitespace-pre-wrap">
          {content}
          <span className="inline-block h-4 w-1.5 animate-pulse bg-foreground/70" />
        </div>
      </div>
    </div>
  )
}
