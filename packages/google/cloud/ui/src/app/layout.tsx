import type { Metadata } from 'next'
import './globals.css'

export const metadata: Metadata = {
  title: 'Cloud Signer',
  description: 'Deploy and sign with ed25519 keys on Google Cloud Run',
}

export default function RootLayout({
  children,
}: {
  children: React.ReactNode
}) {
  return (
    <html lang="en" className="dark">
      <body className="bg-bg text-text min-h-screen">
        {children}
      </body>
    </html>
  )
}
