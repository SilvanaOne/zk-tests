'use client'

import { useState } from 'react'
import * as ed25519 from '@noble/ed25519'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { Card } from '@/shared/ui/card'
import { Copy, Check } from 'lucide-react'

interface DeployResult {
  name: string
  privateKey: string
  publicKey: string
}

interface SignResult {
  name: string
  public_key: string
  signature: string
}

function bytesToBase64(bytes: Uint8Array): string {
  return btoa(String.fromCharCode(...bytes))
}

function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes).map(b => b.toString(16).padStart(2, '0')).join('')
}

function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false)

  const handleCopy = async () => {
    await navigator.clipboard.writeText(text)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  return (
    <button
      onClick={handleCopy}
      className="p-1 rounded hover:bg-bg transition-colors text-muted hover:text-text"
    >
      {copied ? <Check size={14} /> : <Copy size={14} />}
    </button>
  )
}

export default function Home() {
  const [name, setName] = useState('')
  const [deployResult, setDeployResult] = useState<DeployResult | null>(null)
  const [cloudRunUrl, setCloudRunUrl] = useState('')
  const [message, setMessage] = useState('')
  const [signResult, setSignResult] = useState<SignResult | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')

  const handleDeploy = async () => {
    if (!name.trim()) {
      setError('Please enter a name')
      return
    }

    setError('')
    setLoading(true)

    try {
      // Generate ed25519 keypair in browser
      const privateKey = ed25519.utils.randomPrivateKey()
      const publicKey = await ed25519.getPublicKeyAsync(privateKey)

      setDeployResult({
        name: name.trim(),
        privateKey: bytesToBase64(privateKey),
        publicKey: bytesToHex(publicKey),
      })
    } catch (e) {
      setError(`Failed to generate keys: ${e}`)
    } finally {
      setLoading(false)
    }
  }

  const handleSign = async () => {
    if (!cloudRunUrl.trim()) {
      setError('Please enter Cloud Run URL')
      return
    }
    if (!message.trim()) {
      setError('Please enter a message to sign')
      return
    }

    setError('')
    setLoading(true)
    setSignResult(null)

    try {
      const response = await fetch(`${cloudRunUrl.trim()}/sign`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ message: message.trim() }),
      })

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${await response.text()}`)
      }

      const result: SignResult = await response.json()
      setSignResult(result)
    } catch (e) {
      setError(`Failed to sign: ${e}`)
    } finally {
      setLoading(false)
    }
  }

  return (
    <main className="min-h-screen p-8">
      <div className="max-w-2xl mx-auto space-y-8">
        <h1 className="text-2xl font-bold text-accent">Cloud Signer</h1>

        {/* Deploy Section */}
        <Card className="p-6">
          <h2 className="text-lg font-medium mb-4">Deploy Agent</h2>

          <div className="space-y-4">
            <div>
              <label className="block text-sm text-muted mb-1">Name</label>
              <Input
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Enter agent name"
              />
            </div>

            <Button onClick={handleDeploy} disabled={loading} className="w-full">
              {loading ? 'Generating...' : 'Deploy'}
            </Button>
          </div>

          {deployResult && (
            <div className="mt-6 space-y-3">
              <div className="text-sm text-buy font-medium">Keys Generated Successfully</div>

              <div className="space-y-2">
                <div className="flex items-center justify-between gap-2 p-2 bg-bg rounded">
                  <span className="text-xs text-muted">Name</span>
                  <div className="flex items-center gap-2">
                    <code className="text-xs font-mono">{deployResult.name}</code>
                    <CopyButton text={deployResult.name} />
                  </div>
                </div>

                <div className="flex items-center justify-between gap-2 p-2 bg-bg rounded">
                  <span className="text-xs text-muted">Private Key (base64)</span>
                  <div className="flex items-center gap-2">
                    <code className="text-xs font-mono truncate max-w-[200px]">{deployResult.privateKey}</code>
                    <CopyButton text={deployResult.privateKey} />
                  </div>
                </div>

                <div className="flex items-center justify-between gap-2 p-2 bg-bg rounded">
                  <span className="text-xs text-muted">Public Key (hex)</span>
                  <div className="flex items-center gap-2">
                    <code className="text-xs font-mono truncate max-w-[200px]">{deployResult.publicKey}</code>
                    <CopyButton text={deployResult.publicKey} />
                  </div>
                </div>
              </div>

              <div className="mt-4 p-3 bg-bg rounded border border-border">
                <div className="text-xs text-muted mb-2">Next steps:</div>
                <ol className="text-xs text-muted space-y-1 list-decimal list-inside">
                  <li>Create secrets in Google Secret Manager:
                    <ul className="ml-4 mt-1 space-y-1">
                      <li><code className="bg-panel px-1 rounded">SIGNER_NAME</code> = {deployResult.name}</li>
                      <li><code className="bg-panel px-1 rounded">SIGNER_PRIVATE_KEY</code> = {deployResult.privateKey}</li>
                    </ul>
                  </li>
                  <li>Deploy the serverless folder to Cloud Run</li>
                  <li>Copy the Cloud Run URL and use it below to sign messages</li>
                </ol>
              </div>

              <div className="mt-4 p-3 bg-bg rounded border border-border">
                <div className="text-xs text-muted mb-2">Example curl command:</div>
                <code className="text-xs font-mono break-all">
                  curl -X POST https://YOUR-SERVICE.run.app/sign -H &quot;Content-Type: application/json&quot; -d &apos;{`{"message": "hello"}`}&apos;
                </code>
              </div>
            </div>
          )}
        </Card>

        {/* Sign Section */}
        <Card className="p-6">
          <h2 className="text-lg font-medium mb-4">Sign Message</h2>

          <div className="space-y-4">
            <div>
              <label className="block text-sm text-muted mb-1">Cloud Run URL</label>
              <Input
                value={cloudRunUrl}
                onChange={(e) => setCloudRunUrl(e.target.value)}
                placeholder="https://your-service-xxx.run.app"
              />
            </div>

            <div>
              <label className="block text-sm text-muted mb-1">Message</label>
              <Input
                value={message}
                onChange={(e) => setMessage(e.target.value)}
                placeholder="Enter message to sign"
              />
            </div>

            <Button onClick={handleSign} disabled={loading} variant="buy" className="w-full">
              {loading ? 'Signing...' : 'Sign'}
            </Button>
          </div>

          {signResult && (
            <div className="mt-6 space-y-2">
              <div className="text-sm text-buy font-medium">Signature Result</div>

              <div className="flex items-center justify-between gap-2 p-2 bg-bg rounded">
                <span className="text-xs text-muted">Name</span>
                <div className="flex items-center gap-2">
                  <code className="text-xs font-mono">{signResult.name}</code>
                  <CopyButton text={signResult.name} />
                </div>
              </div>

              <div className="flex items-center justify-between gap-2 p-2 bg-bg rounded">
                <span className="text-xs text-muted">Public Key</span>
                <div className="flex items-center gap-2">
                  <code className="text-xs font-mono truncate max-w-[200px]">{signResult.public_key}</code>
                  <CopyButton text={signResult.public_key} />
                </div>
              </div>

              <div className="flex items-center justify-between gap-2 p-2 bg-bg rounded">
                <span className="text-xs text-muted">Signature</span>
                <div className="flex items-center gap-2">
                  <code className="text-xs font-mono truncate max-w-[200px]">{signResult.signature}</code>
                  <CopyButton text={signResult.signature} />
                </div>
              </div>
            </div>
          )}
        </Card>

        {error && (
          <div className="p-3 bg-sell/10 border border-sell rounded-lg text-sell text-sm">
            {error}
          </div>
        )}
      </div>
    </main>
  )
}
