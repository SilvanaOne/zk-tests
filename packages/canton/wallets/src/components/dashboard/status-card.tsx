"use client";

import type React from "react";
import { useState } from "react";
import {
  Card,
  CardHeader,
  CardTitle,
  CardContent,
  CardDescription,
} from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Copy, Eye, EyeOff } from "lucide-react";

interface StatusCardProps {
  title: string;
  icon?: React.ElementType;
  description?: string;
  isLoading?: boolean;
  children: React.ReactNode;
  className?: string;
  titleClassName?: string;
}

export function StatusCard({
  title,
  icon: Icon,
  description,
  isLoading,
  children,
  className,
  titleClassName,
}: StatusCardProps) {
  return (
    <Card
      className={cn(
        "bg-card border border-border rounded-lg shadow-md text-foreground",
        className
      )}
    >
      <CardHeader className="pb-3">
        <div className="flex items-center space-x-2">
          {Icon && (
            <Icon className={cn("h-5 w-5 text-primary", titleClassName)} />
          )}
          <CardTitle className={cn("text-lg font-semibold", titleClassName)}>
            {title}
          </CardTitle>
        </div>
        {description && (
          <CardDescription className="text-muted-foreground text-xs pt-1">
            {description}
          </CardDescription>
        )}
      </CardHeader>
      <CardContent>
        {isLoading ? (
          <div className="space-y-2">
            <Skeleton className="h-5 w-3/4 bg-muted rounded" />
            <Skeleton className="h-5 w-1/2 bg-muted rounded" />
            <Skeleton className="h-5 w-2/3 bg-muted rounded" />
          </div>
        ) : (
          children
        )}
      </CardContent>
    </Card>
  );
}

interface DataRowProps {
  label: string;
  value?: string | number | React.ReactNode;
  isLoading?: boolean;
  isSensitive?: boolean;
  truncate?: boolean;
  className?: string;
  valueClassName?: string;
  disabled?: boolean;
}

export function DataRow({
  label,
  value,
  isLoading,
  isSensitive,
  truncate = true,
  className,
  valueClassName,
  disabled = false,
}: DataRowProps) {
  const [isVisible, setIsVisible] = useState(!isSensitive);

  const displayValue =
    isSensitive && !isVisible ? "••••••••••••••••••••" : value;
  const truncatedValue =
    typeof displayValue === "string" && truncate && displayValue.length > 30
      ? `${displayValue.substring(0, 15)}...${displayValue.substring(
          displayValue.length - 15
        )}`
      : displayValue;

  if (isLoading) {
    return (
      <div
        className={cn(
          "flex justify-between items-center py-1.5 border-b border-border/50",
          className
        )}
      >
        <Skeleton className="h-4 w-1/3 bg-muted rounded" />
        <Skeleton className="h-4 w-1/2 bg-muted rounded" />
      </div>
    );
  }

  return (
    <div
      className={cn(
        "flex flex-col sm:flex-row justify-between sm:items-center py-1.5 border-b border-border/50 last:border-b-0",
        disabled && "opacity-40 cursor-not-allowed",
        className
      )}
    >
      <span
        className={cn(
          "text-xs font-medium mb-0.5 sm:mb-0",
          disabled ? "text-neutral-600" : "text-muted-foreground"
        )}
      >
        {label}:
      </span>
      <div className="flex items-center space-x-1">
        <span
          className={cn(
            "text-xs break-all",
            disabled ? "text-neutral-600" : "text-foreground",
            valueClassName
          )}
        >
          {truncatedValue ?? "N/A"}
        </span>
        {typeof value === "string" && value.length > 0 && !disabled && (
          <Button
            variant="ghost"
            size="icon"
            className="h-5 w-5 text-muted-foreground hover:text-foreground"
            onClick={() => navigator.clipboard.writeText(value)}
          >
            <Copy className="h-3.5 w-3.5" />
            <span className="sr-only">Copy {label}</span>
          </Button>
        )}
        {isSensitive && !disabled && (
          <Button
            variant="ghost"
            size="icon"
            className="h-5 w-5 text-muted-foreground hover:text-foreground"
            onClick={() => setIsVisible(!isVisible)}
          >
            {isVisible ? (
              <EyeOff className="h-3.5 w-3.5" />
            ) : (
              <Eye className="h-3.5 w-3.5" />
            )}
            <span className="sr-only">
              {isVisible ? "Hide" : "Show"} {label}
            </span>
          </Button>
        )}
      </div>
    </div>
  );
}

export function StatusPill({
  status,
  text,
}: {
  status: "success" | "warning" | "error" | "info";
  text: string;
}) {
  const colors = {
    success: "bg-brand-green/20 text-brand-green border-brand-green/50",
    warning: "bg-brand-yellow/20 text-brand-yellow border-brand-yellow/50",
    error: "bg-red-500/20 text-red-400 border-red-500/50",
    info: "bg-brand-blue/20 text-brand-blue border-brand-blue/50",
  };
  return (
    <span
      className={`px-2 py-0.5 text-xs font-medium rounded-full border ${colors[status]}`}
    >
      {text}
    </span>
  );
}
