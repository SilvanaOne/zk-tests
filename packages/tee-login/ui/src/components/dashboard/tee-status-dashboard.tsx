"use client";

import { StatusCard, DataRow, StatusPill } from "./status-card";
import type { TeeStatus } from "@/lib/tee";
import {
  Cpu,
  ShieldCheck,
  Hash,
  KeyRound,
  Server,
  Lock,
  Unlock,
} from "lucide-react";
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@/components/ui/accordion";

const formatBytes = (bytes: number, decimals = 2) => {
  if (bytes === 0) return "0 Bytes";
  const k = 1024;
  const dm = decimals < 0 ? 0 : decimals;
  const sizes = ["Bytes", "KB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return (
    Number.parseFloat((bytes / Math.pow(k, i)).toFixed(dm)) + " " + sizes[i]
  );
};

interface PcrRowProps {
  pcrIndex: string;
  pcrValue: string;
  isLocked?: boolean;
  isLoading?: boolean;
}

function PcrRow({ pcrIndex, pcrValue, isLocked, isLoading }: PcrRowProps) {
  return (
    <div className="flex flex-col sm:flex-row justify-between sm:items-center py-1.5 border-b border-border/50 last:border-b-0">
      <div className="flex items-center space-x-2 mb-1 sm:mb-0">
        {isLocked !== undefined && (
          <div className="flex items-center">
            {isLocked ? (
              <div title="PCR is locked">
                <Lock className="h-4 w-4 text-brand-green" />
              </div>
            ) : (
              <div title="PCR is not locked">
                <Unlock className="h-4 w-4 text-brand-yellow" />
              </div>
            )}
          </div>
        )}
        <span className="text-xs font-medium text-muted-foreground">
          PCR {pcrIndex}:
        </span>
      </div>
      <div className="flex items-center space-x-2">
        <span className="text-xs text-foreground break-all font-mono">
          {pcrValue.length > 24
            ? `${pcrValue.substring(0, 12)}...${pcrValue.substring(
                pcrValue.length - 12
              )}`
            : pcrValue}
        </span>
        <button
          className="h-5 w-5 text-muted-foreground hover:text-foreground flex items-center justify-center"
          onClick={() => navigator.clipboard.writeText(pcrValue)}
          title="Copy PCR value"
        >
          <svg
            className="h-3.5 w-3.5"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"
            />
          </svg>
        </button>
      </div>
    </div>
  );
}

export function TeeStatusDashboard({
  status,
  isLoading,
  sections = [
    "system-resources",
    "attestation-details",
    "pcr-values",
    "tee-addresses",
  ],
  title = "TEE Status",
}: TeeStatus) {
  const attestation = status?.attestation;
  const stats = status?.stats;

  const shouldShowSection = (sectionName: string) =>
    sections.includes(sectionName);

  return (
    <StatusCard
      title={title}
      icon={Server}
      isLoading={isLoading}
      description="Real-time TEE system health and attestation details."
    >
      <div className="space-y-2">
        <DataRow
          isLoading={isLoading}
          label="Connection"
          value={
            attestation?.is_valid ? (
              <StatusPill status="success" text="Secure & Validated" />
            ) : (
              <StatusPill status="error" text="Validation Failed" />
            )
          }
        />

        <Accordion type="multiple" defaultValue={sections} className="w-full">
          {shouldShowSection("system-resources") && (
            <AccordionItem
              value="system-resources"
              className="border-t border-border/50"
            >
              <AccordionTrigger className="text-xs text-muted-foreground hover:no-underline hover:text-foreground py-2.5">
                <div className="flex items-center">
                  <Cpu className="h-4 w-4 mr-2 text-muted-foreground" /> System
                  Resources
                </div>
              </AccordionTrigger>
              <AccordionContent className="pt-1 pb-0 space-y-1">
                <DataRow
                  isLoading={isLoading}
                  label="CPU Cores"
                  value={stats?.cpu_cores}
                />
                <DataRow
                  isLoading={isLoading}
                  label="Total Memory"
                  value={stats?.memory ? formatBytes(stats.memory) : undefined}
                />
                <DataRow
                  isLoading={isLoading}
                  label="Used Memory"
                  value={
                    stats?.used_memory
                      ? formatBytes(stats.used_memory)
                      : undefined
                  }
                />
                <DataRow
                  isLoading={isLoading}
                  label="Available Memory"
                  value={
                    stats?.available_memory
                      ? formatBytes(stats.available_memory)
                      : undefined
                  }
                />
                <DataRow
                  isLoading={isLoading}
                  label="System Timestamp"
                  value={
                    stats?.timestamp
                      ? new Date(stats.timestamp).toLocaleString()
                      : undefined
                  }
                />
              </AccordionContent>
            </AccordionItem>
          )}

          {shouldShowSection("attestation-details") && (
            <AccordionItem
              value="attestation-details"
              className="border-t border-border/50"
            >
              <AccordionTrigger className="text-xs text-muted-foreground hover:no-underline hover:text-foreground py-2.5">
                <div className="flex items-center">
                  <ShieldCheck className="h-4 w-4 mr-2 text-muted-foreground" />{" "}
                  Attestation Details
                </div>
              </AccordionTrigger>
              <AccordionContent className="pt-1 pb-0 space-y-1">
                <DataRow
                  isLoading={isLoading}
                  label="Module ID"
                  value={attestation?.module_id}
                  truncate
                />
                <DataRow
                  isLoading={isLoading}
                  label="Digest Algorithm"
                  value={attestation?.digest}
                />
                <DataRow
                  isLoading={isLoading}
                  label="Attestation Timestamp"
                  value={
                    attestation?.timestamp
                      ? new Date(attestation.timestamp).toLocaleString()
                      : undefined
                  }
                />
              </AccordionContent>
            </AccordionItem>
          )}

          {shouldShowSection("pcr-values") && (
            <AccordionItem
              value="pcr-values"
              className="border-t border-border/50"
            >
              <AccordionTrigger className="text-xs text-muted-foreground hover:no-underline hover:text-foreground py-2.5">
                <div className="flex items-center">
                  <Hash className="h-4 w-4 mr-2 text-muted-foreground" /> PCR
                  Values
                  {attestation?.pcr_locked && (
                    <span className="ml-2 text-xs text-muted-foreground">
                      (
                      {
                        Object.values(attestation.pcr_locked).filter(Boolean)
                          .length
                      }{" "}
                      locked)
                    </span>
                  )}
                </div>
              </AccordionTrigger>
              <AccordionContent className="pt-1 pb-0">
                {attestation?.pcr_map &&
                  Object.entries(attestation.pcr_map).map(
                    ([pcrIndex, pcrValue]) => (
                      <PcrRow
                        key={`pcr-${pcrIndex}`}
                        pcrIndex={pcrIndex}
                        pcrValue={pcrValue}
                        isLocked={attestation.pcr_locked?.[Number(pcrIndex)]}
                        isLoading={isLoading}
                      />
                    )
                  )}
                {(!attestation?.pcr_map ||
                  Object.keys(attestation.pcr_map).length === 0) &&
                  !isLoading && (
                    <p className="text-sm text-muted-foreground py-2">
                      No PCR values available.
                    </p>
                  )}
              </AccordionContent>
            </AccordionItem>
          )}

          {shouldShowSection("tee-addresses") && (
            <AccordionItem
              value="tee-addresses"
              className="border-t border-border/50"
            >
              <AccordionTrigger className="text-xs text-muted-foreground hover:no-underline hover:text-foreground py-2.5">
                <div className="flex items-center">
                  <KeyRound className="h-4 w-4 mr-2 text-muted-foreground" />{" "}
                  TEE Addresses
                </div>
              </AccordionTrigger>
              <AccordionContent className="pt-1 pb-0 space-y-1">
                <DataRow
                  isLoading={isLoading}
                  label="Ethereum Address"
                  value={attestation?.addresses?.ethereum_address}
                  truncate
                />
                <DataRow
                  isLoading={isLoading}
                  label="Solana Address"
                  value={attestation?.addresses?.solana_address}
                  truncate
                />
                <DataRow
                  isLoading={isLoading}
                  label="Sui Address"
                  value={attestation?.addresses?.sui_address}
                  truncate
                />
                <DataRow
                  isLoading={isLoading}
                  label="Mina Address"
                  value={attestation?.addresses?.mina_address}
                  truncate
                />
              </AccordionContent>
            </AccordionItem>
          )}
        </Accordion>
      </div>
    </StatusCard>
  );
}
