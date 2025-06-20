"use client";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Check } from "lucide-react";
import Image from "next/image";
import { walletOptions, WalletOption } from "@/lib/wallet";

interface WalletConnectProps {
  onConnectWallet: (
    chain: "Sui" | "Solana" | "Ethereum",
    walletName: string
  ) => void;
  onConnectSocial: (provider: "Google" | "GitHub") => void;
  connectedWallets?: {
    sui?: boolean;
    solana?: boolean;
    ethereum?: boolean;
  };
  connectedSocials?: {
    google?: boolean;
    github?: boolean;
  };
}

export function WalletConnect({
  onConnectWallet,
  onConnectSocial,
  connectedWallets = {},
  connectedSocials = {},
}: WalletConnectProps) {
  const isConnected = (option: WalletOption) => {
    if (option.type === "wallet") {
      return connectedWallets[
        option.connectionKey as keyof typeof connectedWallets
      ];
    } else {
      return connectedSocials[
        option.connectionKey as keyof typeof connectedSocials
      ];
    }
  };

  return (
    <Card className="bg-card/50 border-border/50 backdrop-blur-md shadow-xl">
      <CardContent className="p-6">
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
          {walletOptions.map((option, index) => {
            const connected = isConnected(option);
            return (
              <Button
                key={`${option.name}-${option.description}-${index}`}
                variant="outline"
                className={`w-full h-32 flex flex-col items-center justify-center space-y-2 transition-all duration-200 transform hover:scale-105 group relative ${
                  connected
                    ? "bg-green-900/30 hover:bg-green-800/40 border-green-500/50 text-green-100"
                    : "bg-muted/70 hover:bg-muted/90 border-border text-foreground hover:text-foreground"
                }`}
                onClick={() => {
                  if (!connected) {
                    if (option.type === "wallet") {
                      onConnectWallet(
                        option.chain as "Sui" | "Solana" | "Ethereum",
                        option.name
                      );
                    } else {
                      onConnectSocial(option.provider as "Google" | "GitHub");
                    }
                  }
                }}
                disabled={connected}
              >
                {connected && (
                  <Badge className="absolute top-2 right-2 bg-green-600 hover:bg-green-600 text-white text-xs px-2 py-1">
                    <Check className="h-3 w-3 mr-1" />
                    Connected
                  </Badge>
                )}
                <div className="relative w-12 h-12 mb-1 transition-transform group-hover:scale-110">
                  <Image
                    src={option.logo || "/placeholder.svg"}
                    alt={`${option.name} logo`}
                    fill
                    className="object-contain"
                    sizes="48px"
                  />
                </div>
                <div className="text-center">
                  <div className="text-sm font-semibold">{option.name}</div>
                  <div className="text-xs text-muted-foreground">
                    {option.description}
                  </div>
                </div>
              </Button>
            );
          })}
        </div>
      </CardContent>
    </Card>
  );
}
