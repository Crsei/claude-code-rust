import { ChatPanel } from '@/components/chat/ChatPanel'

export default function App() {
  return (
    <div className="flex h-screen w-screen overflow-hidden">
      {/* Main chat area */}
      <main className="flex flex-1 flex-col">
        <header className="flex h-12 items-center border-b border-border px-4">
          <h1 className="text-sm font-semibold text-foreground">cc-rust</h1>
          <span className="ml-2 text-xs text-muted-foreground">Web UI</span>
        </header>
        <ChatPanel />
      </main>
    </div>
  )
}
