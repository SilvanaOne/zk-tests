"use client";

import { motion, AnimatePresence } from "framer-motion";
import { X, Globe, Check } from "lucide-react";

export type NetworkType = "devnet" | "testnet" | "mainnet";

interface NetworkOption {
  id: NetworkType;
  name: string;
  description: string;
  color: string;
  gradient: string;
}

const networkOptions: NetworkOption[] = [
  {
    id: "devnet",
    name: "Devnet",
    description: "Development network for testing",
    color: "text-brand-green",
    gradient: "from-brand-green/40 via-brand-green/30 to-brand-green/20",
  },
  {
    id: "testnet",
    name: "Testnet",
    description: "Test network with test tokens",
    color: "text-brand-yellow",
    gradient: "from-brand-yellow/40 via-brand-yellow/30 to-brand-yellow/20",
  },
  {
    id: "mainnet",
    name: "Mainnet",
    description: "Production network with real assets",
    color: "text-brand-pink",
    gradient: "from-brand-pink/40 via-brand-pink/30 to-brand-pink/20",
  },
];

interface NetworkButtonProps {
  network: NetworkOption;
  selected: boolean;
  onClick: () => void;
}

function NetworkButton({ network, selected, onClick }: NetworkButtonProps) {
  return (
    <motion.button
      initial={{ scale: 0.9, opacity: 0 }}
      animate={{ scale: 1, opacity: 1 }}
      transition={{
        type: "spring",
        duration: 0.35,
        stiffness: 300,
        damping: 25,
      }}
      whileHover={{ y: -3, scale: 1.02 }}
      whileTap={{ scale: 0.98 }}
      onClick={onClick}
      className={`group relative w-full rounded-2xl p-4
                 backdrop-blur-md transition-all duration-300
                 focus:outline-none focus:ring-2 focus:ring-brand-pink focus:ring-offset-2 focus:ring-offset-transparent
                 overflow-hidden border-2
                 ${
                   selected
                     ? `bg-gradient-to-br ${network.gradient} border-white/60 shadow-xl`
                     : `bg-gradient-to-br from-white/10 via-white/5 to-white/10 border-white/30 hover:border-white/50 hover:shadow-xl`
                 }`}
    >
      {/* Bright animated gradient background */}
      <div className="absolute inset-0 bg-gradient-to-br from-brand-pink/10 via-brand-purple/10 to-brand-blue/10 opacity-0 group-hover:opacity-100 transition-opacity duration-300" />

      <div className="relative z-10 flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div
            className={`w-10 h-10 flex items-center justify-center bg-white/10 rounded-lg backdrop-blur-sm ${network.color}`}
          >
            <Globe className="w-5 h-5" />
          </div>
          <div className="text-left">
            <div className="text-base font-semibold text-foreground drop-shadow-md">
              {network.name}
            </div>
            <div className="text-xs text-foreground/70">
              {network.description}
            </div>
          </div>
        </div>

        {/* Selected Indicator */}
        {selected && (
          <div className="w-6 h-6 bg-gradient-to-br from-brand-green via-brand-green to-brand-green/80 rounded-full flex items-center justify-center border-2 border-white shadow-xl shadow-brand-green/50">
            <Check className="w-3 h-3 text-white" strokeWidth={3} />
          </div>
        )}
      </div>

      {/* Bright shine effect on hover */}
      <div className="absolute inset-0 rounded-2xl opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none bg-gradient-to-r from-transparent via-white/20 to-transparent transform -skew-x-12 translate-x-[-100%] group-hover:translate-x-[100%] duration-1000" />
    </motion.button>
  );
}

interface NetworkSelectModalProps {
  isOpen: boolean;
  onClose: () => void;
  onConnect: (network: NetworkType) => void;
  selectedNetwork: NetworkType;
}

export function NetworkSelectModal({
  isOpen,
  onClose,
  onConnect,
  selectedNetwork,
}: NetworkSelectModalProps) {
  return (
    <AnimatePresence>
      {isOpen && (
        <>
          {/* Vibrant Backdrop */}
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.2 }}
            className="fixed inset-0 z-50 bg-gradient-to-br from-brand-pink/20 via-brand-purple/30 to-brand-blue/20 backdrop-blur-sm"
            onClick={onClose}
          />

          {/* Modal */}
          <div className="fixed inset-0 z-50 flex items-center justify-center p-4 sm:p-0">
            <motion.div
              initial={{ scale: 0.9, opacity: 0 }}
              animate={{ scale: 1, opacity: 1 }}
              exit={{ scale: 0.9, opacity: 0 }}
              transition={{
                type: "spring",
                duration: 0.3,
                stiffness: 300,
                damping: 30,
              }}
              className="relative w-full max-w-[400px] sm:max-h-[80vh] sm:rounded-3xl
                         max-sm:fixed max-sm:bottom-0 max-sm:left-0 max-sm:right-0 max-sm:rounded-t-3xl max-sm:rounded-b-none
                         bg-gradient-to-br from-brand-pink/30 via-brand-purple/25 to-brand-blue/30
                         backdrop-blur-xl border-2 border-white/40
                         shadow-[0_32px_96px_rgba(239,69,207,0.4)]
                         overflow-hidden"
            >
              {/* Bright animated gradient overlay */}
              <div className="absolute inset-0 bg-gradient-to-br from-brand-pink/10 via-brand-purple/20 to-brand-blue/10 animate-pulse" />

              {/* Header */}
              <div className="px-6 py-6 border-b border-white/30 relative z-10">
                <div className="flex items-center justify-between">
                  <div className="text-center flex-1">
                    <h2 className="text-2xl md:text-3xl font-bold text-foreground font-sans mb-2 drop-shadow-lg">
                      <span className="bg-gradient-to-r from-brand-pink via-brand-purple to-brand-blue bg-clip-text text-transparent">
                        Select Network
                      </span>
                    </h2>
                    <p
                      className="text-sm text-white max-w-[340px] mx-auto font-medium drop-shadow-lg rounded-lg px-3 py-2 border border-white/40 shadow-xl"
                      style={{
                        background:
                          "linear-gradient(to right, rgba(239, 69, 207, 0.7), rgba(141, 117, 255, 0.7), rgba(95, 168, 255, 0.7))",
                      }}
                    >
                      Choose a Canton network to connect your Loop wallet
                    </p>
                  </div>
                  <button
                    onClick={onClose}
                    className="absolute top-4 right-4 p-2 text-pink-500/80 hover:text-foreground transition-all duration-200 rounded-xl hover:bg-white/20 hover:shadow-lg hover:scale-110"
                  >
                    <X className="w-5 h-5" />
                  </button>
                </div>
              </div>

              {/* Network Options */}
              <div className="flex flex-col gap-3 p-6 relative z-10">
                {networkOptions.map((network) => (
                  <NetworkButton
                    key={network.id}
                    network={network}
                    selected={selectedNetwork === network.id}
                    onClick={() => onConnect(network.id)}
                  />
                ))}
              </div>
            </motion.div>
          </div>
        </>
      )}
    </AnimatePresence>
  );
}
