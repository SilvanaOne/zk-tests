"use client";

import { ShieldCheck, Wifi, WifiOff, Sun, Moon, Loader2 } from "lucide-react";
import { useTheme } from "next-themes";
import { useEffect, useState } from "react";

interface ModernHeaderProps {
  teeConnected: boolean;
  teeLoading: boolean;
  onAddConnection: () => void;
}

export function ModernHeader({
  teeConnected,
  teeLoading,
  onAddConnection,
}: ModernHeaderProps) {
  const { theme, setTheme, resolvedTheme, systemTheme } = useTheme();

  const toggleTheme = () => {
    setTheme(resolvedTheme === "dark" ? "light" : "dark");
  };

  return (
    <header className="fixed top-0 inset-x-0 h-14 z-50 backdrop-blur-md bg-white/5 border-b border-white/10 flex items-center justify-between px-6">
      <div className="flex items-center space-x-3">
        <ShieldCheck className="h-6 w-6 text-brand-pink" />
        <h1 className="text-xl font-semibold text-gradient">
          Silvana TEE Login
        </h1>
      </div>

      <div className="flex items-center space-x-4">
        <div className="flex items-center space-x-2">
          {teeLoading ? (
            <Loader2 className="h-4 w-4 text-brand-yellow animate-spin" />
          ) : teeConnected ? (
            <Wifi className="h-4 w-4 text-brand-green" />
          ) : (
            <WifiOff className="h-4 w-4 text-danger" />
          )}
          <span
            className={`text-xs font-medium ${
              teeConnected
                ? "text-brand-green"
                : teeLoading
                ? "text-brand-yellow"
                : "text-danger"
            }`}
          >
            TEE:{" "}
            {teeLoading
              ? "Connecting..."
              : teeConnected
              ? "Connected"
              : "Disconnected"}
          </span>
        </div>

        {/* Theme Toggle Button */}
        <button
          onClick={toggleTheme}
          className="p-2 rounded-lg bg-white/10 hover:bg-white/20 transition-all duration-200 border border-white/20 hover:border-white/30"
          title={`Switch to ${
            resolvedTheme === "dark" ? "light" : "dark"
          } mode`}
        >
          {resolvedTheme === "dark" ? (
            <Sun className="h-4 w-4 text-brand-yellow" />
          ) : (
            <Moon className="h-4 w-4 text-brand-purple" />
          )}
        </button>

        <button
          onClick={onAddConnection}
          id="add-connection"
          className="
    relative flex items-center gap-2
    h-12 px-6
    rounded-full
    bg-gradient-to-r from-brand-pink via-brand-purple to-brand-blue
    text-white font-semibold
    shadow-[0_4px_20px_rgba(0,0,0,0.2)]
    hover:brightness-105 hover:shadow-[0_6px_24px_rgba(0,0,0,0.25)]
    active:scale-95 transition-all duration-200
  "
        >
          <span className="flex h-6 w-6 items-center justify-center rounded-full bg-white/15">
            <svg viewBox="0 0 24 24" className="h-4 w-4">
              <path
                fill="currentColor"
                d="M12 5v14m7-7H5"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
              />
            </svg>
          </span>
          Add Connection
        </button>
      </div>
    </header>
  );
}
