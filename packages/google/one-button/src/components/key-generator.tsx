"use client";

import { useState } from "react";
import nacl from "tweetnacl";
import bs58 from "bs58";
import { Key, Rocket, RefreshCw, Terminal } from "lucide-react";
import { CopyButton } from "./copy-button";
import { ThemeToggle } from "./theme-toggle";
import { cn } from "@/lib/cn";

interface Keypair {
  publicKey: string;
  privateKey: string;
}

export function KeyGenerator() {
  const [keypair, setKeypair] = useState<Keypair | null>(null);
  const githubRepo = process.env.NEXT_PUBLIC_GITHUB_REPO || "";

  const generateKeypair = () => {
    const kp = nacl.sign.keyPair();
    setKeypair({
      publicKey: bs58.encode(kp.publicKey),
      privateKey: bs58.encode(kp.secretKey),
    });
  };

  const repoName = githubRepo.split('/').pop()?.replace('.git', '') || 'repo';
  const deployCommand = `cd ~ && rm -rf ${repoName} && git clone ${githubRepo} && cd ${repoName} && bash deploy.sh`;
  const cloudShellUrl = "https://shell.cloud.google.com/cloudshell/editor?shellonly=true";

  const handleDeploy = () => {
    window.open(cloudShellUrl, "_blank");
  };

  return (
    <div className="min-h-screen flex flex-col">
      <header className="flex justify-end p-4">
        <ThemeToggle />
      </header>

      <main className="flex-1 flex items-center justify-center p-4">
        <div className="w-full max-w-lg">
          <div className="bg-panel rounded-lg border p-6 shadow-sm">
            <h1 className="text-2xl font-semibold mb-2">Deploy to Cloud Run</h1>
            <p className="text-muted mb-6">
              Generate an ed25519 keypair and deploy your signing server to Google Cloud Run.
            </p>

            {!keypair ? (
              <button
                onClick={generateKeypair}
                className={cn(
                  "w-full flex items-center justify-center gap-2 py-3 px-4 rounded-md",
                  "bg-accent text-white font-medium",
                  "hover:opacity-90 transition-opacity"
                )}
              >
                <Key className="w-5 h-5" />
                Generate Keypair
              </button>
            ) : (
              <div className="space-y-4">
                <div>
                  <label className="text-sm text-muted mb-1 block">Public Key</label>
                  <div className="flex items-center gap-2 bg-bg rounded-md border p-3">
                    <code className="flex-1 text-sm break-all">{keypair.publicKey}</code>
                    <CopyButton text={keypair.publicKey} />
                  </div>
                </div>

                <div>
                  <label className="text-sm text-muted mb-1 block">Private Key</label>
                  <div className="flex items-center gap-2 bg-bg rounded-md border p-3">
                    <code className="flex-1 text-sm break-all">{keypair.privateKey}</code>
                    <CopyButton text={keypair.privateKey} />
                  </div>
                </div>

                <div className="pt-2 space-y-3">
                  <button
                    onClick={handleDeploy}
                    disabled={!githubRepo}
                    className={cn(
                      "w-full flex items-center justify-center gap-2 py-3 px-4 rounded-md",
                      "bg-accent text-white font-medium",
                      "hover:opacity-90 transition-opacity",
                      "disabled:opacity-50 disabled:cursor-not-allowed"
                    )}
                  >
                    <Rocket className="w-5 h-5" />
                    Deploy to Google Cloud Run
                  </button>

                  {!githubRepo && (
                    <p className="text-sm text-error text-center">
                      NEXT_PUBLIC_GITHUB_REPO is not set
                    </p>
                  )}

                  <button
                    onClick={generateKeypair}
                    className={cn(
                      "w-full flex items-center justify-center gap-2 py-2 px-4 rounded-md",
                      "border text-muted",
                      "hover:bg-border/30 transition-colors"
                    )}
                  >
                    <RefreshCw className="w-4 h-4" />
                    Generate New Keypair
                  </button>
                </div>

                <div className="pt-2">
                  <label className="text-sm text-muted mb-1 block">
                    <Terminal className="w-4 h-4 inline mr-1" />
                    Paste this command in Cloud Shell:
                  </label>
                  <div className="flex items-center gap-2 bg-bg rounded-md border p-3">
                    <code className="flex-1 text-xs break-all">{deployCommand}</code>
                    <CopyButton text={deployCommand} />
                  </div>
                  <p className="text-sm text-muted pt-2">
                    Paste your Private Key when prompted. Build logs will be visible.
                  </p>
                </div>
              </div>
            )}
          </div>
        </div>
      </main>
    </div>
  );
}
