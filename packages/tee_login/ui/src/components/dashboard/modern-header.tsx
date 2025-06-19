"use client";

import { ShieldCheck, Wifi, WifiOff, Sun, Moon } from "lucide-react";
import { useTheme } from "next-themes";
import { useEffect, useState } from "react";

interface ModernHeaderProps {
  teeConnected: boolean;
  onAddConnection: () => void;
}

export function ModernHeader({
  teeConnected,
  onAddConnection,
}: ModernHeaderProps) {
  const { theme, setTheme, resolvedTheme, systemTheme } = useTheme();
  const [mounted, setMounted] = useState(false);
  const [actualTheme, setActualTheme] = useState<string>("dark");

  // More robust theme detection
  useEffect(() => {
    setMounted(true);

    // Check multiple sources for theme
    const detectTheme = () => {
      // 1. Check HTML class
      const htmlClass = document.documentElement.classList.contains("dark")
        ? "dark"
        : "light";

      // 2. Check system preference
      const systemPref = window.matchMedia("(prefers-color-scheme: dark)")
        .matches
        ? "dark"
        : "light";

      // 3. Use resolvedTheme if available
      const detected = resolvedTheme || htmlClass || systemPref || "dark";

      setActualTheme(detected);
      console.log("Theme detection:", {
        resolvedTheme,
        htmlClass,
        systemPref,
        detected,
      });
    };

    // Initial detection
    detectTheme();

    // Listen for theme changes
    const observer = new MutationObserver(detectTheme);
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["class"],
    });

    // Listen for system theme changes
    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    mediaQuery.addEventListener("change", detectTheme);

    return () => {
      observer.disconnect();
      mediaQuery.removeEventListener("change", detectTheme);
    };
  }, [resolvedTheme]);

  const toggleTheme = () => {
    setTheme(actualTheme === "dark" ? "light" : "dark");
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
          {teeConnected ? (
            <Wifi className="h-4 w-4 text-brand-green" />
          ) : (
            <WifiOff className="h-4 w-4 text-danger" />
          )}
          <span
            className={`text-xs font-medium ${
              teeConnected ? "text-brand-green" : "text-danger"
            }`}
          >
            TEE: {teeConnected ? "Connected" : "Disconnected"}
          </span>
        </div>

        {/* Theme button hidden temporarily */}

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
