"use client";

import { motion } from "framer-motion";
import { cn } from "@/lib/utils";
import type React from "react";

interface ModernCardProps {
  children: React.ReactNode;
  className?: string;
  delay?: number;
}

export function ModernCard({
  children,
  className,
  delay = 0,
}: ModernCardProps) {
  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{
        type: "spring",
        duration: 0.4,
        delay,
        stiffness: 300,
        damping: 30,
      }}
      whileHover={{ y: -2 }}
      className={cn(
        "bg-card/50 backdrop-blur-lg border border-border/50 rounded-2xl",
        "shadow-[0_4px_40px_rgba(0,0,0,0.25)] hover:shadow-[0_6px_48px_rgba(0,0,0,0.28)]",
        "transition-all duration-300 p-6 md:p-8 lg:p-10",
        className
      )}
    >
      {children}
    </motion.div>
  );
}

interface SectionHeaderProps {
  children: React.ReactNode;
  className?: string;
}

export function SectionHeader({ children, className }: SectionHeaderProps) {
  return (
    <div className={cn("flex items-center mb-6", className)}>
      <span className="inline-block h-4 w-1 bg-brand-pink rounded-full mr-2"></span>
      <h2 className="text-lg font-semibold text-foreground">{children}</h2>
    </div>
  );
}
