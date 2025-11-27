import { ShieldCheck, Wifi, WifiOff } from "lucide-react"

interface HeaderProps {
  teeConnected: boolean
}

export function Header({ teeConnected }: HeaderProps) {
  return (
    <header className="flex flex-col sm:flex-row justify-between items-center py-3 mb-4 border-b border-border">
      <div className="flex items-center space-x-2 mb-2 sm:mb-0">
        <ShieldCheck className="h-8 w-8 text-primary" />
        <h1 className="text-2xl font-bold bg-gradient-to-r from-silvana-gradientFrom to-silvana-gradientTo text-transparent bg-clip-text">
          Silvana Wallet Connect
        </h1>
      </div>
      <div className="flex items-center space-x-1.5">
        {teeConnected ? (
          <Wifi className="h-4 w-4 text-silvana-success" />
        ) : (
          <WifiOff className="h-4 w-4 text-silvana-error" />
        )}
        <span className={`text-xs font-medium ${teeConnected ? "text-silvana-success" : "text-silvana-error"}`}>
          {teeConnected ? "Connected" : "Disconnected"}
        </span>
      </div>
    </header>
  )
}
