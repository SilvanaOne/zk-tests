'use client'

import { useState } from 'react'
import * as ed25519 from '@noble/ed25519'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { Card } from '@/shared/ui/card'
import { Copy, Check, ExternalLink } from 'lucide-react'

const CLOUD_RUN_BUTTON_URL = 'https://deploy.cloud.run/?git_repo=https://github.com/SilvanaOne/zk-tests&dir=packages/google/cloud/serverless'

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

function CopyButton({ text, label }: { text: string; label?: string }) {
  const [copied, setCopied] = useState(false)

  const handleCopy = async () => {
    await navigator.clipboard.writeText(text)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  return (
    <button
      onClick={handleCopy}
      className="inline-flex items-center gap-1 px-2 py-1 rounded bg-accent/10 hover:bg-accent/20 transition-colors text-accent text-xs font-medium"
    >
      {copied ? <Check size={12} /> : <Copy size={12} />}
      {label || (copied ? 'Copied!' : 'Copy')}
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

  const handleGenerateKeys = async () => {
    if (!name.trim()) {
      setError('Please enter a name')
      return
    }

    setError('')
    setLoading(true)

    try {
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

  const handleDeployToCloud = () => {
    window.open(CLOUD_RUN_BUTTON_URL, '_blank')
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

            <Button onClick={handleGenerateKeys} disabled={loading || !!deployResult} className="w-full">
              {loading ? 'Generating...' : deployResult ? 'Keys Generated' : 'Generate Keys'}
            </Button>
          </div>

          {deployResult && (
            <div className="mt-6 space-y-4">
              <div className="text-sm text-buy font-medium">Keys Generated - Follow the steps below to deploy:</div>

              {/* Step 1 */}
              <div className="p-4 bg-bg rounded-lg border border-border">
                <div className="flex items-center justify-between mb-2">
                  <span className="text-sm font-medium">Step 1: Copy SIGNER_NAME</span>
                  <CopyButton text={deployResult.name} />
                </div>
                <code className="block text-xs font-mono bg-panel p-2 rounded break-all">
                  {deployResult.name}
                </code>
              </div>

              {/* Step 2 */}
              <div className="p-4 bg-bg rounded-lg border border-border">
                <div className="flex items-center justify-between mb-2">
                  <span className="text-sm font-medium">Step 2: Copy SIGNER_PRIVATE_KEY</span>
                  <CopyButton text={deployResult.privateKey} />
                </div>
                <code className="block text-xs font-mono bg-panel p-2 rounded break-all">
                  {deployResult.privateKey}
                </code>
              </div>

              {/* Step 3 */}
              <div className="p-4 bg-bg rounded-lg border border-border">
                <div className="mb-2">
                  <span className="text-sm font-medium">Step 3: Deploy to Google Cloud</span>
                </div>
                <p className="text-xs text-muted mb-3">
                  Click the button below. When prompted, paste the values from Steps 1 and 2.
                </p>
                <Button onClick={handleDeployToCloud} variant="buy" className="w-full">
                  <ExternalLink size={16} className="mr-2" />
                  Deploy to Google Cloud
                </Button>
              </div>

              {/* Public Key (for reference) */}
              <div className="p-3 bg-bg rounded border border-border">
                <div className="flex items-center justify-between">
                  <span className="text-xs text-muted">Public Key (hex) - for verification</span>
                  <CopyButton text={deployResult.publicKey} />
                </div>
                <code className="block text-xs font-mono mt-1 truncate">
                  {deployResult.publicKey}
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
                placeholder="https://cloud-signer-xxx.run.app"
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
