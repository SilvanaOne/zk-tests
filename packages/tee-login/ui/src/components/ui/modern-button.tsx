"use client";

import { motion } from "framer-motion";
import { cn } from "@/lib/utils";
import type React from "react";

interface ModernButtonProps {
  children: React.ReactNode;
  intent?: "primary" | "secondary" | "icon";
  className?: string;
  onClick?: () => void;
  disabled?: boolean;
  type?: "button" | "submit";
}

export function ModernButton({
  children,
  intent = "primary",
  className,
  onClick,
  disabled = false,
  type = "button",
}: ModernButtonProps) {
  const baseClasses =
    "font-medium transition-all duration-200 focus:outline-none focus:ring-2 focus:ring-brand focus:ring-offset-2";

  const intentClasses = {
    primary:
      "bg-brand hover:bg-brand/90 text-white rounded-full px-5 py-2.5 shadow-md",
    secondary:
      "border border-white/10 bg-white/5 hover:bg-white/10 text-foreground rounded-full px-5 py-2.5",
    icon: "w-10 h-10 rounded-full bg-white/5 hover:bg-white/10 flex items-center justify-center hover:rotate-6",
  };

  return (
    <motion.button
      type={type}
      onClick={onClick}
      disabled={disabled}
      whileTap={{ scale: 0.95 }}
      className={cn(
        baseClasses,
        intentClasses[intent],
        disabled && "opacity-50 cursor-not-allowed",
        "min-h-[44px] min-w-[44px]", // WCAG tap target
        className
      )}
    >
      {children}
    </motion.button>
  );
}
