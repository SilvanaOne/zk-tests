"use client";

import { useState, useEffect, useCallback } from "react";
import Image from "next/image";
import { StatusCard, DataRow, StatusPill } from "./status-card";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  Activity,
  Edit3,
  CheckCircle,
  XCircle,
  Loader2,
  Coins,
  RefreshCw,
  ShieldCheck,
  Send,
  UserCheck,
  ExternalLink,
  Download,
} from "lucide-react";
import { Input } from "@/components/ui/input";
import { getLoopHoldings, getLoopActiveContracts, signLoopMessage, verifyLoopSignature, getLoopPublicKey, verifyPartyIdMatchesPublicKey, transferCC, createTransferPreapprovalProposal, signLoopTransactionHash, type TransferResult, type PreapprovalResult } from "@/lib/loop";
import { createPhantomPreapprovalProposal } from "@/lib/phantom-preapproval";
import { createSolflarePreapprovalProposal } from "@/lib/solflare-preapproval";
import { createSolflareTransfer } from "@/lib/solflare-transfer";
import { acceptAdvancedPaymentRequest } from "@/lib/solflare-accept-request";
import { acceptAdvancedPaymentRequestLoop, type AcceptResult } from "@/lib/loop-accept-request";
import { createUserServiceRequest } from "@/lib/request-service";
import { acceptCredentialOffer } from "@/lib/accept-credential-offer";
import { signSolflareTransactionHash } from "@/lib/solflare";
import { fetchContractDetails, decodeCIP56HoldingBlob, decodeCredentialOfferBlob, decodeCredentialBlob, decodeCredentialBillingBlob, decodeContractBlob, type TransferPreapprovalCreateArguments, type AmuletCreateArguments, type ContractDetails, type CIP56HoldingCreateArguments, type CredentialOfferCreateArguments, type CredentialCreateArguments, type CredentialBillingCreateArguments } from "@/lib/blob";
import { getHoldingsFromLedger, getActiveContractsFromLedger, getPreapprovalsFromLedger } from "@/lib/ledger-api";
import type { LedgerHolding, LedgerActiveContract } from "@/lib/ledger-api-types";
import type { Holding } from "@fivenorth/loop-sdk";
import { fetchAllTokenMetadata, getTokenMetadata, type TokenMetadata } from "@/lib/token-metadata";

// Type for preapproval contract with decoded details
interface PreapprovalContract {
  contractId: string;
  templateId: string;
  provider: string;
  receiver: string;
  expiresAt?: string;
  createdAt?: string;
}

// Type for UserService contract
interface UserServiceContract {
  contractId: string;
  templateId: string;
  operator: string;
  user: string;
  dso: string;
}

// Type for UserServiceRequest contract
interface UserServiceRequestContract {
  contractId: string;
  templateId: string;
  operator: string;
  user: string;
}

// Type for CredentialOffer contract
interface CredentialOfferContract {
  contractId: string;
  templateId: string;
  operator: string;
  issuer: string;
  holder: string;
  dso: string;
  id: string;
  description: string;
  billingParams?: {
    feePerDayUsd?: { rate: string };
    billingPeriodMinutes?: string;
    depositTargetAmountUsd?: string;
  };
  depositInitialAmountUsd?: string;
}

// Type for Credential contract (active subscription)
interface CredentialContract {
  contractId: string;
  templateId: string;
  issuer: string;
  holder: string;
  id: string;
  description: string;
  validFrom?: string;
  validUntil?: string;
  claims: Array<{
    subject: string;
    property: string;
    value: string;
  }>;
}

// Type for CredentialBilling contract
interface CredentialBillingContract {
  contractId: string;
  templateId: string;
  operator: string;
  issuer: string;
  holder: string;
  dso: string;
  credentialId: string;
  params: {
    feePerDayUsd: { rate: string };
    billingPeriodMinutes: string;
    depositTargetAmountUsd: string;
  };
  balanceState: {
    currentDepositAmountCc: string;
    totalUserDepositCc: string;
    totalCredentialFeesPaidCc: string;
  };
  billingState: {
    status: "New" | "Success" | "Failure";
    billedUntil: string;
    createdAt: string;
  };
}

// Type for Amulet contract with decoded details from Lighthouse API
interface AmuletContract {
  contractId: string;
  templateId: string;
  packageName: string;
  owner: string;
  dso: string;
  amount: AmuletCreateArguments["amount"];
  createdAt?: string;
  createdEventBlob: string;  // Base64-encoded protobuf blob for disclosed contracts
  blob: ContractDetails | null;  // Decoded contract details from Lighthouse API
}

// Type for CIP-56 compatible holding contract (with Lighthouse API data)
interface CIP56Holding {
  contractId: string;
  templateId: string;
  packageName: string;
  createdAt?: string;
  createdEventBlob: string;
  // Data from Lighthouse API create_arguments
  owner: string;
  instrumentId: string;      // The instrument ID (e.g., "WBTC")
  instrumentAdmin: string;   // The admin/registrar party
  amount: string;
  label: string;
  isLocked: boolean;
}

interface LoopWalletDashboardProps {
  loopPartyId?: string | null;
  network?: "devnet" | "testnet" | "mainnet";
  walletName?: string;
  walletType?: "loop" | "phantom" | "solflare" | null;
}

export function LoopWalletDashboard({ loopPartyId, network = "devnet", walletName = "Loop", walletType = "loop" }: LoopWalletDashboardProps) {
  const isConnected = !!loopPartyId;
  const [messageToSign, setMessageToSign] = useState("Hello Loop world!");
  const [signedMessage, setSignedMessage] = useState<{
    message: string;
    signature: string;
  } | null>(null);
  const [signStatus, setSignStatus] = useState<
    "idle" | "loading" | "success" | "error"
  >("idle");
  const [verifyStatus, setVerifyStatus] = useState<
    "idle" | "loading" | "valid" | "invalid"
  >("idle");

  // Sign Multihash state (Solflare only)
  const [multihashToSign, setMultihashToSign] = useState("");
  const [signedMultihash, setSignedMultihash] = useState<{
    multihash: string;
    signature: string;
  } | null>(null);
  const [signMultihashStatus, setSignMultihashStatus] = useState<
    "idle" | "loading" | "success" | "error"
  >("idle");

  // Holdings state
  const [holdings, setHoldings] = useState<Holding[]>([]);
  const [holdingsLoading, setHoldingsLoading] = useState(false);
  const [holdingsError, setHoldingsError] = useState<string | null>(null);

  // Active Contracts state (enriched with Lighthouse API data)
  const [amuletContracts, setAmuletContracts] = useState<AmuletContract[]>([]);
  const [contractsLoading, setContractsLoading] = useState(false);
  const [contractsError, setContractsError] = useState<string | null>(null);

  // PartyId verification state
  const [partyIdVerified, setPartyIdVerified] = useState<boolean | null>(null);

  // Transfer state
  const [transferReceiver, setTransferReceiver] = useState("");
  const [transferAmount, setTransferAmount] = useState("");
  const [transferDescription, setTransferDescription] = useState("");
  const [transferStatus, setTransferStatus] = useState<"idle" | "awaiting" | "loading" | "success" | "error">("idle");
  const [transferResult, setTransferResult] = useState<TransferResult | null>(null);

  // Preapproval state
  const [preapprovalProvider, setPreapprovalProvider] = useState("");
  const [preapprovalStatus, setPreapprovalStatus] = useState<"idle" | "awaiting" | "loading" | "success" | "error">("idle");
  const [preapprovalResult, setPreapprovalResult] = useState<PreapprovalResult | null>(null);

  // Preapproval contracts state (from Loop SDK)
  const [acceptedPreapprovals, setAcceptedPreapprovals] = useState<PreapprovalContract[]>([]);
  const [preapprovalContractsLoading, setPreapprovalContractsLoading] = useState(false);

  // Accept Advanced Payment Request state (Solflare only)
  const [acceptRequestCid, setAcceptRequestCid] = useState("");
  const [acceptRequestStatus, setAcceptRequestStatus] = useState<"idle" | "awaiting" | "loading" | "success" | "error">("idle");
  const [acceptRequestResult, setAcceptRequestResult] = useState<AcceptResult | null>(null);

  // Request Service state
  const [requestServiceOperator, setRequestServiceOperator] = useState("");
  const [requestServiceStatus, setRequestServiceStatus] = useState<"idle" | "loading" | "success" | "error">("idle");
  const [requestServiceResult, setRequestServiceResult] = useState<{
    success: boolean;
    contractId?: string;
    updateId?: string;
    error?: string;
  } | null>(null);

  // User Service Status state
  const [userServices, setUserServices] = useState<UserServiceContract[]>([]);
  const [userServiceRequests, setUserServiceRequests] = useState<UserServiceRequestContract[]>([]);
  const [userServiceStatusLoading, setUserServiceStatusLoading] = useState(false);

  // Credential Offers state
  const [credentialOffers, setCredentialOffers] = useState<CredentialOfferContract[]>([]);
  const [acceptingOfferId, setAcceptingOfferId] = useState<string | null>(null);
  const [acceptOfferResult, setAcceptOfferResult] = useState<{
    success: boolean;
    credentialCid?: string;
    error?: string;
  } | null>(null);

  // Active Credentials state (accepted subscriptions)
  const [credentials, setCredentials] = useState<CredentialContract[]>([]);
  const [credentialBillings, setCredentialBillings] = useState<CredentialBillingContract[]>([]);

  // CIP-56 Holdings state
  const [cip56Holdings, setCip56Holdings] = useState<CIP56Holding[]>([]);
  const [cip56HoldingsLoading, setCip56HoldingsLoading] = useState(false);
  const [cip56HoldingsError, setCip56HoldingsError] = useState<string | null>(null);

  // Token metadata state (for CIP-56 token images)
  const [tokenMetadataMap, setTokenMetadataMap] = useState<Map<string, TokenMetadata>>(new Map());
  const [tokenMetadataLoaded, setTokenMetadataLoaded] = useState(false);

  // Ledger API holdings state (for Solflare/Phantom)
  const [ledgerHoldings, setLedgerHoldings] = useState<LedgerHolding[]>([]);
  const [ledgerContracts, setLedgerContracts] = useState<LedgerActiveContract[]>([]);

  // Capitalize first letter for display
  const networkDisplay = network.charAt(0).toUpperCase() + network.slice(1);

  // Fetch holdings when connected
  const fetchHoldings = useCallback(async () => {
    console.log("[LoopWallet] fetchHoldings called, isConnected:", isConnected, "walletType:", walletType);
    if (!isConnected || !loopPartyId) return;

    setHoldingsLoading(true);
    setHoldingsError(null);
    try {
      // Use Ledger API for Solflare/Phantom, Loop SDK for Loop wallet
      if (walletType === "solflare" || walletType === "phantom") {
        console.log("[LoopWallet] Using Ledger API for holdings...");
        const ledgerResult = await getHoldingsFromLedger(loopPartyId);
        console.log("[LoopWallet] Ledger holdings result:", ledgerResult);
        setLedgerHoldings(ledgerResult);
        setHoldings([]); // Clear Loop holdings

        // Also derive CIP-56 holdings from ledger result (non-CC tokens)
        const cip56FromLedger = ledgerResult.filter(h => h.tokenId !== "CC");
        const cip56Holdings: CIP56Holding[] = cip56FromLedger.map(h => ({
          contractId: h.contractId,
          templateId: h.templateId,
          packageName: h.templateId.split(":")[0] || "",
          createdAt: undefined,
          createdEventBlob: "",
          owner: "",
          instrumentId: h.tokenId,
          instrumentAdmin: "",
          amount: h.amount,
          label: h.tokenId,
          isLocked: h.isLocked,
        }));
        setCip56Holdings(cip56Holdings);
      } else {
        const result = await getLoopHoldings();
        console.log("[LoopWallet] Holdings result:", result);
        if (result) {
          setHoldings(result);
        } else {
          setHoldings([]);
        }
        setLedgerHoldings([]); // Clear Ledger holdings
      }
    } catch (error: any) {
      console.error("[LoopWallet] Failed to fetch holdings:", error);
      setHoldingsError(error?.message || "Failed to fetch holdings");
    } finally {
      setHoldingsLoading(false);
    }
  }, [isConnected, loopPartyId, walletType]);

  // Fetch active contracts when connected (Amulet contracts enriched with Lighthouse API)
  const fetchActiveContracts = useCallback(async () => {
    console.log("[LoopWallet] fetchActiveContracts called, isConnected:", isConnected, "walletType:", walletType);
    if (!isConnected || !loopPartyId) return;

    setContractsLoading(true);
    setContractsError(null);
    try {
      // Use Ledger API for Solflare/Phantom, Loop SDK for Loop wallet
      if (walletType === "solflare" || walletType === "phantom") {
        console.log("[LoopWallet] Using Ledger API for active contracts...");
        const allContracts = await getActiveContractsFromLedger(loopPartyId);
        console.log("[LoopWallet] Ledger contracts result:", allContracts);
        setLedgerContracts(allContracts);

        // Filter for Amulet contracts and convert to AmuletContract format
        const amulets = allContracts.filter(c => c.templateId.includes("Splice.Amulet:Amulet"));
        const enrichedContracts: AmuletContract[] = amulets.map(contract => ({
          contractId: contract.contractId,
          templateId: contract.templateId,
          packageName: contract.templateId.split(":")[0] || "",
          owner: contract.createArgument?.owner || "",
          dso: contract.createArgument?.dso || "",
          amount: contract.createArgument?.amulet?.amount || contract.createArgument?.amount || { initialAmount: "0", createdAt: { number: "0" }, ratePerRound: { rate: "0" } },
          createdAt: undefined,
          createdEventBlob: contract.createdEventBlob || "",
          blob: null,
        }));
        setAmuletContracts(enrichedContracts);
      } else {
        const result = await getLoopActiveContracts({
          templateId: "#splice-amulet:Splice.Amulet:Amulet"
        });
        console.log("[LoopWallet] Active contracts result:", result);

        if (result && result.length > 0) {
          // Fetch details from Lighthouse API for each contract
          const enrichedContracts: AmuletContract[] = await Promise.all(
            result.map(async (contract) => {
              const createdEvent = contract.contractEntry?.JsActiveContract?.createdEvent;
              const contractId = createdEvent?.contractId || createdEvent?.contract_id || "";

              // Fetch full details from Lighthouse API
              const details = await fetchContractDetails(contractId, network);
              const args = details?.create_arguments as AmuletCreateArguments | undefined;

              return {
                contractId,
                templateId: details?.template_id || createdEvent?.templateId || "",
                packageName: details?.package_name || createdEvent?.packageName || "",
                owner: args?.owner || "",
                dso: args?.dso || "",
                amount: args?.amount || { initialAmount: "0", createdAt: { number: "0" }, ratePerRound: { rate: "0" } },
                createdAt: createdEvent?.createdAt,
                createdEventBlob: createdEvent?.createdEventBlob || "",
                blob: details,
              };
            })
          );

          console.log("[LoopWallet] Enriched Amulet contracts:", enrichedContracts);
          setAmuletContracts(enrichedContracts);
        } else {
          setAmuletContracts([]);
        }
        setLedgerContracts([]); // Clear Ledger contracts
      }
    } catch (error: any) {
      console.error("[LoopWallet] Failed to fetch active contracts:", error);
      setContractsError(error?.message || "Failed to fetch contracts");
    } finally {
      setContractsLoading(false);
    }
  }, [isConnected, loopPartyId, walletType, network]);

  // Fetch preapproval contracts from Loop SDK and enrich with Lighthouse API
  const fetchPreapprovalContracts = useCallback(async () => {
    console.log("[LoopWallet] fetchPreapprovalContracts called, isConnected:", isConnected, "partyId:", loopPartyId, "walletType:", walletType);
    if (!isConnected || !loopPartyId) return;

    setPreapprovalContractsLoading(true);
    try {
      // Use Ledger API for Solflare/Phantom, Loop SDK for Loop wallet
      if (walletType === "solflare" || walletType === "phantom") {
        console.log("[LoopWallet] Using Ledger API for preapprovals...");
        const ledgerPreapprovals = await getPreapprovalsFromLedger(loopPartyId);
        console.log("[LoopWallet] Ledger preapprovals result:", ledgerPreapprovals);

        const preapprovals: PreapprovalContract[] = ledgerPreapprovals
          .filter(c => c.templateId.includes("TransferPreapproval:TransferPreapproval"))
          .map(contract => ({
            contractId: contract.contractId,
            templateId: contract.templateId,
            provider: contract.createArgument?.provider || "",
            receiver: contract.createArgument?.receiver || loopPartyId,
            expiresAt: contract.createArgument?.expiresAt,
            createdAt: undefined,
          }));

        console.log("[LoopWallet] Mapped ledger preapprovals:", preapprovals);
        setAcceptedPreapprovals(preapprovals);
      } else {
        // Fetch TransferPreapproval contracts directly from Loop SDK
        // This gives us all preapprovals where user is receiver (has visibility)
        const contracts = await getLoopActiveContracts({
          templateId: "#splice-amulet:Splice.AmuletRules:TransferPreapproval"
        });

        console.log("[LoopWallet] TransferPreapproval contracts from Loop SDK:", contracts);

        if (contracts && contracts.length > 0) {
          // Extract contract IDs and fetch details from Lighthouse API
          const preapprovals: PreapprovalContract[] = await Promise.all(
            contracts.map(async (contract) => {
              const createdEvent = contract.contractEntry?.JsActiveContract?.createdEvent;
              const contractId = createdEvent?.contractId || createdEvent?.contract_id || "";

              // Fetch full details from Lighthouse API
              const details = await fetchContractDetails(contractId, network);
              const args = details?.create_arguments as TransferPreapprovalCreateArguments | undefined;

              return {
                contractId,
                templateId: createdEvent?.templateId || createdEvent?.template_id || contract.template_id,
                provider: args?.provider || "",
                receiver: args?.receiver || loopPartyId,
                expiresAt: args?.expiresAt,
                createdAt: createdEvent?.createdAt,
              };
            })
          );

          console.log("[LoopWallet] Mapped preapprovals with Lighthouse details:", preapprovals);
          setAcceptedPreapprovals(preapprovals);
        } else {
          console.log("[LoopWallet] No TransferPreapproval contracts found");
          setAcceptedPreapprovals([]);
        }
      }
    } catch (error: any) {
      console.error("[LoopWallet] Failed to fetch preapproval contracts:", error);
      setAcceptedPreapprovals([]);
    } finally {
      setPreapprovalContractsLoading(false);
    }
  }, [isConnected, loopPartyId, walletType, network]);

  // Fetch CIP-56 compatible holdings using interface filter
  // Note: For Solflare/Phantom, CIP-56 holdings are derived in fetchHoldings
  const fetchCIP56Holdings = useCallback(async () => {
    console.log("[LoopWallet] fetchCIP56Holdings called, isConnected:", isConnected, "walletType:", walletType);
    if (!isConnected || !loopPartyId) return;

    // Skip for Solflare/Phantom - CIP-56 holdings are derived from ledger holdings in fetchHoldings
    if (walletType === "solflare" || walletType === "phantom") {
      console.log("[LoopWallet] Skipping CIP-56 fetch for Solflare/Phantom (handled by fetchHoldings)");
      return;
    }

    setCip56HoldingsLoading(true);
    setCip56HoldingsError(null);
    try {
      // Query contracts implementing the CIP-56 Holding interface
      const result = await getLoopActiveContracts({
        interfaceId: "#splice-api-token-holding-v1:Splice.Api.Token.HoldingV1:Holding"
      });
      console.log("[LoopWallet] CIP-56 Holdings result:", result);

      if (result && result.length > 0) {
        // Decode contract data from createdEventBlob using protobuf
        const holdings: CIP56Holding[] = result.map((contract) => {
          const createdEvent = contract.contractEntry?.JsActiveContract?.createdEvent;
          const contractId = createdEvent?.contractId || createdEvent?.contract_id || "";
          const createdEventBlob = createdEvent?.createdEventBlob || "";

          // Decode the blob directly using protobuf (no Lighthouse API needed)
          const args = createdEventBlob ? decodeCIP56HoldingBlob(createdEventBlob) : null;

          console.log("[LoopWallet] Decoded CIP-56 holding blob:", { contractId, args });

          return {
            contractId,
            templateId: createdEvent?.templateId || "",
            packageName: createdEvent?.packageName || "",
            createdAt: createdEvent?.createdAt,
            createdEventBlob,
            owner: args?.owner || "",
            instrumentId: args?.instrument?.id || "",
            instrumentAdmin: args?.instrument?.admin || args?.registrar || "",
            amount: args?.amount || "0",
            label: args?.label || "",
            isLocked: args?.lock !== null && args?.lock !== undefined,
          };
        });

        console.log("[LoopWallet] Parsed CIP-56 holdings from blob:", holdings);
        setCip56Holdings(holdings);
      } else {
        setCip56Holdings([]);
      }
    } catch (error: any) {
      console.error("[LoopWallet] Failed to fetch CIP-56 holdings:", error);
      setCip56HoldingsError(error?.message || "Failed to fetch CIP-56 holdings");
    } finally {
      setCip56HoldingsLoading(false);
    }
  }, [isConnected, loopPartyId, walletType]);

  // Fetch token metadata for CIP-56 holdings (logo URLs, names, etc.)
  const fetchTokenMetadata = useCallback(async () => {
    if (tokenMetadataLoaded) return;

    console.log("[LoopWallet] Fetching token metadata...");
    try {
      const metadata = await fetchAllTokenMetadata();
      console.log("[LoopWallet] Token metadata result:", metadata);

      // Build a map keyed by instrumentAdmin::instrumentId
      const metadataMap = new Map<string, TokenMetadata>();
      for (const token of metadata) {
        const key = `${token.instrumentAdmin}::${token.instrumentId}`;
        metadataMap.set(key, token);
      }
      setTokenMetadataMap(metadataMap);
      setTokenMetadataLoaded(true);
    } catch (error) {
      console.error("[LoopWallet] Failed to fetch token metadata:", error);
    }
  }, [tokenMetadataLoaded]);

  // Helper to get token metadata for a CIP-56 holding
  const getHoldingMetadata = useCallback((holding: CIP56Holding): TokenMetadata | undefined => {
    const key = `${holding.instrumentAdmin}::${holding.instrumentId}`;
    return tokenMetadataMap.get(key);
  }, [tokenMetadataMap]);

  // Fetch UserService and UserServiceRequest contracts
  const fetchUserServiceStatus = useCallback(async () => {
    console.log("[LoopWallet] fetchUserServiceStatus called, isConnected:", isConnected, "walletType:", walletType);
    if (!isConnected || !loopPartyId) return;

    // Only for Loop wallet (uses Loop SDK to query contracts)
    if (walletType !== "loop") {
      console.log("[LoopWallet] Skipping user service status for non-Loop wallet");
      return;
    }

    setUserServiceStatusLoading(true);
    try {
      const packageName = process.env.NEXT_PUBLIC_UTILITY_CREDENTIAL_PACKAGE_NAME || "utility-credential-app-v0";

      // Fetch UserService contracts
      const serviceContracts = await getLoopActiveContracts({
        templateId: `#${packageName}:Utility.Credential.App.V0.Service.User:UserService`
      });

      console.log("[LoopWallet] UserService contracts:", serviceContracts);

      if (serviceContracts && serviceContracts.length > 0) {
        const services: UserServiceContract[] = serviceContracts.map(contract => {
          const createdEvent = contract.contractEntry?.JsActiveContract?.createdEvent;
          const blob = createdEvent?.createdEventBlob || contract.createdEventBlob || "";

          // Try to decode the blob to get operator
          let operator = "";
          let user = "";
          let dso = "";

          if (blob) {
            const decoded = decodeContractBlob(blob);
            if (decoded && decoded.fields) {
              const fields = decoded.fields.fields;
              // UserService fields: operator (0), user (1), dso (2)
              if (fields.length >= 3) {
                operator = fields[0]?.value?.sum.case === "party" ? fields[0].value.sum.value : "";
                user = fields[1]?.value?.sum.case === "party" ? fields[1].value.sum.value : "";
                dso = fields[2]?.value?.sum.case === "party" ? fields[2].value.sum.value : "";
              }
            }
          }

          // Fallback to createArguments
          const createArgs = createdEvent?.createArguments || {};
          console.log("[LoopWallet] UserService decoded - operator:", operator, "user:", user);

          return {
            contractId: createdEvent?.contractId || createdEvent?.contract_id || "",
            templateId: createdEvent?.templateId || "",
            operator: operator || createArgs.operator || "",
            user: user || createArgs.user || "",
            dso: dso || createArgs.dso || "",
          };
        });
        setUserServices(services);
      } else {
        setUserServices([]);
      }

      // Fetch UserServiceRequest contracts
      const requestContracts = await getLoopActiveContracts({
        templateId: `#${packageName}:Utility.Credential.App.V0.Service.User:UserServiceRequest`
      });

      console.log("[LoopWallet] UserServiceRequest contracts:", requestContracts);

      if (requestContracts && requestContracts.length > 0) {
        const requests: UserServiceRequestContract[] = requestContracts.map(contract => {
          const createdEvent = contract.contractEntry?.JsActiveContract?.createdEvent;
          const createArgs = createdEvent?.createArguments || {};
          return {
            contractId: createdEvent?.contractId || createdEvent?.contract_id || "",
            templateId: createdEvent?.templateId || "",
            operator: createArgs.operator || "",
            user: createArgs.user || "",
          };
        });
        setUserServiceRequests(requests);
      } else {
        setUserServiceRequests([]);
      }

      // Fetch CredentialOffer contracts (where user is the holder)
      const offerContracts = await getLoopActiveContracts({
        templateId: `#${packageName}:Utility.Credential.App.V0.Model.Offer:CredentialOffer`
      });

      console.log("[LoopWallet] CredentialOffer contracts:", offerContracts);

      if (offerContracts && offerContracts.length > 0) {
        const offers: CredentialOfferContract[] = offerContracts.map(contract => {
          const createdEvent = contract.contractEntry?.JsActiveContract?.createdEvent;
          const contractId = createdEvent?.contractId || createdEvent?.contract_id || contract.contract_id || "";
          const templateId = createdEvent?.templateId || "";
          const blob = createdEvent?.createdEventBlob || contract.createdEventBlob || "";

          // Try to decode the blob first
          if (blob) {
            const decoded = decodeCredentialOfferBlob(blob);
            if (decoded) {
              console.log("[LoopWallet] Decoded CredentialOffer from blob:", decoded);
              return {
                contractId,
                templateId,
                operator: decoded.operator,
                issuer: decoded.issuer,
                holder: decoded.holder,
                dso: decoded.dso,
                id: decoded.id,
                description: decoded.description,
                billingParams: decoded.billingParams,
                depositInitialAmountUsd: decoded.depositInitialAmountUsd,
              };
            }
          }

          // Fallback to createArguments if blob decoding fails
          const args = createdEvent?.createArguments || contract.payload || {};
          console.log("[LoopWallet] CredentialOffer fallback args:", JSON.stringify(args, null, 2));
          return {
            contractId,
            templateId,
            operator: args.operator || "",
            issuer: args.issuer || "",
            holder: args.holder || "",
            dso: args.dso || "",
            id: args.id || "",
            description: args.description || "",
            billingParams: args.billingParams,
            depositInitialAmountUsd: args.depositInitialAmountUsd,
          };
        });
        console.log("[LoopWallet] Mapped offers:", offers);
        setCredentialOffers(offers);
      } else {
        setCredentialOffers([]);
      }

      // Fetch active Credential contracts (accepted credentials)
      const credentialContracts = await getLoopActiveContracts({
        templateId: `#utility-credential-v0:Utility.Credential.V0.Credential:Credential`
      });

      console.log("[LoopWallet] Credential contracts:", credentialContracts);

      if (credentialContracts && credentialContracts.length > 0) {
        const creds: CredentialContract[] = credentialContracts.map(contract => {
          const createdEvent = contract.contractEntry?.JsActiveContract?.createdEvent;
          const contractId = createdEvent?.contractId || createdEvent?.contract_id || contract.contract_id || "";
          const templateId = createdEvent?.templateId || "";
          const blob = createdEvent?.createdEventBlob || contract.createdEventBlob || "";

          // Try to decode the blob first
          if (blob) {
            const decoded = decodeCredentialBlob(blob);
            if (decoded) {
              console.log("[LoopWallet] Decoded Credential from blob:", decoded);
              return {
                contractId,
                templateId,
                issuer: decoded.issuer,
                holder: decoded.holder,
                id: decoded.id,
                description: decoded.description,
                validFrom: decoded.validFrom,
                validUntil: decoded.validUntil,
                claims: decoded.claims,
              };
            }
          }

          // Fallback to createArguments if blob decoding fails
          const args = createdEvent?.createArguments || contract.payload || {};
          return {
            contractId,
            templateId,
            issuer: args.issuer || "",
            holder: args.holder || "",
            id: args.id || "",
            description: args.description || "",
            validFrom: args.validFrom,
            validUntil: args.validUntil,
            claims: args.claims || [],
          };
        });
        console.log("[LoopWallet] Mapped credentials:", creds);
        setCredentials(creds);
      } else {
        setCredentials([]);
      }

      // Fetch CredentialBilling contracts
      const billingContracts = await getLoopActiveContracts({
        templateId: `#${packageName}:Utility.Credential.App.V0.Model.Billing:CredentialBilling`
      });

      console.log("[LoopWallet] CredentialBilling contracts:", billingContracts);

      if (billingContracts && billingContracts.length > 0) {
        const billings: CredentialBillingContract[] = billingContracts.map(contract => {
          const createdEvent = contract.contractEntry?.JsActiveContract?.createdEvent;
          const contractId = createdEvent?.contractId || createdEvent?.contract_id || contract.contract_id || "";
          const templateId = createdEvent?.templateId || "";
          const blob = createdEvent?.createdEventBlob || contract.createdEventBlob || "";

          // Try to decode the blob first
          if (blob) {
            const decoded = decodeCredentialBillingBlob(blob);
            if (decoded) {
              console.log("[LoopWallet] Decoded CredentialBilling from blob:", decoded);
              return {
                contractId,
                templateId,
                operator: decoded.operator,
                issuer: decoded.issuer,
                holder: decoded.holder,
                dso: decoded.dso,
                credentialId: decoded.credentialId,
                params: decoded.params,
                balanceState: {
                  currentDepositAmountCc: decoded.balanceState.currentDepositAmountCc,
                  totalUserDepositCc: decoded.balanceState.totalUserDepositCc,
                  totalCredentialFeesPaidCc: decoded.balanceState.totalCredentialFeesPaidCc,
                },
                billingState: {
                  status: decoded.billingState.status,
                  billedUntil: decoded.billingState.billedUntil,
                  createdAt: decoded.billingState.createdAt,
                },
              };
            }
          }

          // Fallback to createArguments if blob decoding fails
          const args = createdEvent?.createArguments || contract.payload || {};
          return {
            contractId,
            templateId,
            operator: args.operator || "",
            issuer: args.issuer || "",
            holder: args.holder || "",
            dso: args.dso || "",
            credentialId: args.credentialId || "",
            params: args.params || { feePerDayUsd: { rate: "0" }, billingPeriodMinutes: "0", depositTargetAmountUsd: "0" },
            balanceState: args.balanceState || { currentDepositAmountCc: "0", totalUserDepositCc: "0", totalCredentialFeesPaidCc: "0" },
            billingState: args.billingState || { status: "New", billedUntil: "", createdAt: "" },
          };
        });
        console.log("[LoopWallet] Mapped credential billings:", billings);
        setCredentialBillings(billings);
      } else {
        setCredentialBillings([]);
      }
    } catch (error: any) {
      console.error("[LoopWallet] Failed to fetch user service status:", error);
      setUserServices([]);
      setUserServiceRequests([]);
      setCredentialOffers([]);
      setCredentials([]);
      setCredentialBillings([]);
    } finally {
      setUserServiceStatusLoading(false);
    }
  }, [isConnected, loopPartyId, walletType]);

  useEffect(() => {
    if (isConnected) {
      fetchHoldings();
      fetchActiveContracts();
      fetchPreapprovalContracts();
      fetchCIP56Holdings();
      fetchUserServiceStatus();
      fetchTokenMetadata();
    }
  }, [isConnected, fetchHoldings, fetchActiveContracts, fetchPreapprovalContracts, fetchCIP56Holdings, fetchUserServiceStatus, fetchTokenMetadata]);

  // Verify partyId matches public key when connected
  useEffect(() => {
    const verifyPartyId = async () => {
      const publicKey = getLoopPublicKey();
      if (loopPartyId && publicKey) {
        const isValid = await verifyPartyIdMatchesPublicKey(loopPartyId, publicKey);
        setPartyIdVerified(isValid);
      } else {
        setPartyIdVerified(null);
      }
    };
    verifyPartyId();
  }, [loopPartyId]);

  // Wallet URLs by network
  const walletUrls: Record<string, string> = {
    devnet: "https://devnet.cantonloop.com",
    testnet: "https://testnet.cantonloop.com",
    mainnet: "https://cantonloop.com",
  };

  // Open Loop wallet in new tab (or focus if already open with same name)
  const openLoopWallet = () => {
    window.open(walletUrls[network] || walletUrls.devnet, "loop-wallet");
  };

  // Format balance - the API returns decimal strings like "10000.000000"
  const formatBalance = (amount: string): string => {
    const num = parseFloat(amount);
    if (isNaN(num)) return "0";
    // Format with up to 4 decimal places, removing trailing zeros
    return num.toLocaleString(undefined, {
      minimumFractionDigits: 0,
      maximumFractionDigits: 4,
    });
  };

  // Download contract data as JSON file for use as disclosed contract in transactions
  const downloadContractJson = (contract: AmuletContract) => {
    const data = {
      templateId: contract.templateId,
      contractId: contract.contractId,
      createdEventBlob: contract.createdEventBlob,  // Base64-encoded protobuf for disclosed contracts
      create_arguments: contract.blob?.create_arguments,  // Decoded payload for reference
    };
    const blob = new Blob([JSON.stringify(data, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `contract-${contract.contractId.slice(0, 8)}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const handleSignMessage = async () => {
    if (!messageToSign || !isConnected) return;
    setSignStatus("loading");
    try {
      const result = await signLoopMessage(messageToSign);
      console.log("[LoopWallet] Sign message result:", result);
      if (result) {
        setSignedMessage({
          message: messageToSign,
          signature: typeof result === "string" ? result : JSON.stringify(result),
        });
        setSignStatus("success");
      } else {
        setSignStatus("error");
      }
    } catch (error: any) {
      console.error("[LoopWallet] Sign message error:", error);
      setSignStatus("error");
    }
    setMessageToSign("");
    setVerifyStatus("idle");
  };

  const handleVerifySignature = async () => {
    if (!signedMessage) return;
    setVerifyStatus("loading");
    try {
      const isValid = await verifyLoopSignature(
        signedMessage.message,
        signedMessage.signature
      );
      setVerifyStatus(isValid ? "valid" : "invalid");
    } catch (error) {
      console.error("[LoopWallet] Verification error:", error);
      setVerifyStatus("invalid");
    }
  };

  // Sign Multihash handler (Solflare and Loop)
  const handleSignMultihash = async () => {
    if (!multihashToSign || (walletType !== "solflare" && walletType !== "loop")) return;

    setSignMultihashStatus("loading");
    setSignedMultihash(null);

    try {
      let signature: string | undefined;

      if (walletType === "solflare") {
        signature = await signSolflareTransactionHash(multihashToSign);
      } else if (walletType === "loop") {
        signature = await signLoopTransactionHash(multihashToSign);
      }

      if (signature) {
        setSignedMultihash({
          multihash: multihashToSign,
          signature,
        });
        setSignMultihashStatus("success");
      } else {
        setSignMultihashStatus("error");
      }
    } catch (error) {
      console.error("[LoopWallet] Sign multihash error:", error);
      setSignMultihashStatus("error");
    }
  };

  const handleTransfer = async () => {
    if (!transferReceiver || !transferAmount || !isConnected || !loopPartyId) return;

    setTransferStatus("awaiting");
    setTransferResult(null);

    try {
      let result: TransferResult;

      if (walletType === "phantom") {
        // Phantom transfer not yet supported due to signing restrictions
        console.log("[LoopWallet] Phantom transfer not supported");
        result = { success: false, error: "Transfer CC is not yet supported for Phantom wallet. Please use Solflare or Loop wallet." };
      } else if (walletType === "solflare") {
        // Use Solflare signing flow (interactive submission)
        console.log("[LoopWallet] Using Solflare transfer flow");
        result = await createSolflareTransfer({
          senderPartyId: loopPartyId,
          receiverPartyId: transferReceiver,
          amount: transferAmount,
          description: transferDescription || undefined,
        });
      } else {
        // Use Loop SDK flow (standard submission)
        console.log("[LoopWallet] Using Loop transfer flow");
        result = await transferCC({
          receiver: transferReceiver,
          amount: transferAmount,
          description: transferDescription || undefined,
        });
      }

      setTransferResult(result);
      setTransferStatus(result.success ? "success" : "error");

      if (result.success) {
        // Clear form on success
        setTransferReceiver("");
        setTransferAmount("");
        setTransferDescription("");
        // Refresh holdings after successful transfer
        fetchHoldings();
        fetchActiveContracts();
      }
    } catch (error: any) {
      console.error("[LoopWallet] Transfer error:", error);
      setTransferResult({ success: false, error: error?.message || "Transfer failed" });
      setTransferStatus("error");
    }
  };

  const handleCreatePreapproval = async () => {
    if (!isConnected || !preapprovalProvider.trim() || !loopPartyId) return;

    setPreapprovalStatus("awaiting");
    setPreapprovalResult(null);

    try {
      let result: PreapprovalResult;

      if (walletType === "phantom") {
        // Use Phantom signing flow (interactive submission)
        console.log("[LoopWallet] Using Phantom preapproval flow");
        result = await createPhantomPreapprovalProposal({
          phantomPartyId: loopPartyId,
          provider: preapprovalProvider.trim()
        });
      } else if (walletType === "solflare") {
        // Use Solflare signing flow (interactive submission)
        console.log("[LoopWallet] Using Solflare preapproval flow");
        result = await createSolflarePreapprovalProposal({
          solflarePartyId: loopPartyId,
          provider: preapprovalProvider.trim()
        });
      } else {
        // Use Loop SDK flow (standard submission)
        console.log("[LoopWallet] Using Loop preapproval flow");
        result = await createTransferPreapprovalProposal({
          provider: preapprovalProvider.trim()
        });
      }

      setPreapprovalResult(result);
      setPreapprovalStatus(result.success ? "success" : "error");

      if (result.success) {
        // Clear input and refresh preapproval contracts after successful creation
        setPreapprovalProvider("");
        fetchPreapprovalContracts();
      }
    } catch (error: any) {
      console.error("[LoopWallet] Preapproval error:", error);
      setPreapprovalResult({ success: false, error: error?.message || "Failed to create preapproval" });
      setPreapprovalStatus("error");
    }
  };

  // Handle Accept Advanced Payment Request (Solflare and Loop)
  const handleAcceptRequest = async () => {
    if (!isConnected || !acceptRequestCid.trim() || !loopPartyId) return;
    if (walletType !== "solflare" && walletType !== "loop") return;

    setAcceptRequestStatus("awaiting");
    setAcceptRequestResult(null);

    try {
      // Get package ID from env (provider hint is handled server-side via PARTY_SETTLEMENT_OPERATOR)
      const packageId = process.env.NEXT_PUBLIC_ADVANCED_PAYMENT_PACKAGE_ID ||
        "b688939b6beb9b42866b0a0c34a5fb8e03c54a3e3e8c4ba3fe5a9ff9c31780f2";

      console.log("[LoopWallet] Accepting advanced payment request...");
      console.log("[LoopWallet] Request CID:", acceptRequestCid.trim());
      console.log("[LoopWallet] Party ID:", loopPartyId);
      console.log("[LoopWallet] Package ID:", packageId);
      console.log("[LoopWallet] Wallet Type:", walletType);

      let result: AcceptResult;

      if (walletType === "loop") {
        // Use Loop wallet's hybrid flow (interactive submission + message signing)
        result = await acceptAdvancedPaymentRequestLoop({
          loopPartyId,
          requestContractId: acceptRequestCid.trim(),
          packageId,
        });
      } else {
        // Use Solflare's flow
        result = await acceptAdvancedPaymentRequest({
          senderPartyId: loopPartyId,
          requestContractId: acceptRequestCid.trim(),
          packageId,
        });
      }

      setAcceptRequestResult(result);
      setAcceptRequestStatus(result.success ? "success" : "error");

      if (result.success) {
        // Clear input and refresh holdings after successful accept
        setAcceptRequestCid("");
        fetchHoldings();
        fetchActiveContracts();
      }
    } catch (error: any) {
      console.error("[LoopWallet] Accept request error:", error);
      setAcceptRequestResult({ success: false, error: error?.message || "Failed to accept request" });
      setAcceptRequestStatus("error");
    }
  };

  // Handle Request Service
  const handleRequestService = async () => {
    if (!isConnected || !requestServiceOperator.trim() || !loopPartyId) return;
    if (walletType !== "solflare" && walletType !== "loop") return;

    setRequestServiceStatus("loading");
    setRequestServiceResult(null);

    try {
      const result = await createUserServiceRequest({
        userPartyId: loopPartyId,
        operatorPartyId: requestServiceOperator.trim(),
        walletType,
      });

      setRequestServiceResult(result);
      setRequestServiceStatus(result.success ? "success" : "error");

      if (result.success) {
        setRequestServiceOperator("");
        fetchUserServiceStatus(); // Refresh the list
      }
    } catch (error: any) {
      console.error("[LoopWallet] Request service error:", error);
      setRequestServiceResult({ success: false, error: error?.message || "Failed to create request" });
      setRequestServiceStatus("error");
    }
  };

  // Handle Accept Credential Offer
  const handleAcceptCredentialOffer = async (offer: CredentialOfferContract) => {
    if (!isConnected || !loopPartyId || walletType !== "loop") return;

    // Find the UserService contract for this offer (matching operator)
    const userService = userServices.find(s => s.operator === offer.operator);
    if (!userService) {
      setAcceptOfferResult({ success: false, error: "No matching UserService found for this operator" });
      return;
    }

    setAcceptingOfferId(offer.contractId);
    setAcceptOfferResult(null);

    try {
      const isPaid = !!offer.billingParams || !!offer.depositInitialAmountUsd;

      // For paid credentials, the function fetches amulets and context automatically
      // Pass issuer to match correct FeaturedAppRight on mainnet (issuer is the FeaturedAppRight provider)
      const result = await acceptCredentialOffer({
        userPartyId: loopPartyId,
        userServiceCid: userService.contractId,
        credentialOfferCid: offer.contractId,
        isPaid,
        depositAmountUsd: offer.depositInitialAmountUsd ? parseFloat(offer.depositInitialAmountUsd) : undefined,
        issuerParty: offer.issuer,
      });

      setAcceptOfferResult(result);

      if (result.success) {
        fetchUserServiceStatus(); // Refresh the list
      }
    } catch (error: any) {
      console.error("[LoopWallet] Accept credential offer error:", error);
      setAcceptOfferResult({ success: false, error: error?.message || "Failed to accept offer" });
    } finally {
      setAcceptingOfferId(null);
    }
  };

  if (!isConnected) {
    return (
      <StatusCard
        title={`${walletName} Wallet`}
        icon={Activity}
        description={`Connect a wallet to manage your ${walletName} assets.`}
        className="h-full"
      >
        <div className="text-center py-4">
          <StatusPill
            status="info"
            text={`${walletName} Wallet Not Active`}
          />
          <p className="text-sm text-muted-foreground mt-2">
            Please connect to activate wallet functionalities.
          </p>
        </div>
      </StatusCard>
    );
  }

  return (
    <StatusCard
      title={`${walletName} Wallet`}
      icon={Activity}
      description={`Manage your ${walletName} assets securely.`}
      className="h-full"
    >
      <div className="space-y-4">
        <DataRow
          label="Network"
          value={networkDisplay}
          truncate={false}
        />
        {loopPartyId && (
          <div className="flex items-center gap-2">
            <DataRow
              label="Party ID"
              value={loopPartyId}
              truncate={false}
              className="flex-1"
            />
            {partyIdVerified === true && (
              <span title="Verified: Public key matches Party ID">
                <CheckCircle className="h-4 w-4 text-brand-green flex-shrink-0" />
              </span>
            )}
            {partyIdVerified === false && (
              <span title="Verification failed">
                <XCircle className="h-4 w-4 text-destructive flex-shrink-0" />
              </span>
            )}
          </div>
        )}
        {getLoopPublicKey() && (
          <div className="flex items-center gap-2">
            <DataRow
              label="Public Key"
              value={getLoopPublicKey()!}
              truncate={false}
              className="flex-1"
            />
            {partyIdVerified === true && (
              <span title="Verified: Public key matches Party ID">
                <CheckCircle className="h-4 w-4 text-brand-green flex-shrink-0" />
              </span>
            )}
            {partyIdVerified === false && (
              <span title="Verification failed">
                <XCircle className="h-4 w-4 text-destructive flex-shrink-0" />
              </span>
            )}
          </div>
        )}

        {/* Holdings Section */}
        <div className="space-y-2 p-3 rounded-md bg-muted/30 border border-border backdrop-blur-sm">
          <div className="flex items-center justify-between">
            <h4 className="text-sm font-semibold text-foreground flex items-center">
              <Coins className="w-4 h-4 mr-2" />
              Holdings
            </h4>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={() => fetchHoldings()}
              disabled={holdingsLoading}
              className="h-6 w-6 p-0"
            >
              <RefreshCw className={`h-3 w-3 ${holdingsLoading ? "animate-spin" : ""}`} />
            </Button>
          </div>

          {holdingsLoading && (
            <div className="flex items-center justify-center py-4">
              <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
            </div>
          )}

          {holdingsError && (
            <Alert variant="destructive" className="p-2">
              <XCircle className="h-4 w-4" />
              <AlertDescription className="text-xs">{holdingsError}</AlertDescription>
            </Alert>
          )}

          {!holdingsLoading && !holdingsError && holdings.length === 0 && ledgerHoldings.length === 0 && (
            <p className="text-xs text-muted-foreground text-center py-2">
              No holdings found
            </p>
          )}

          {/* Loop SDK Holdings (Loop wallet) */}
          {!holdingsLoading && !holdingsError && holdings.length > 0 && (
            <div className="space-y-2">
              {holdings.map((holding, index) => (
                <div
                  key={`${holding.instrument_id.admin}-${holding.instrument_id.id}-${index}`}
                  className="flex items-center gap-3 p-2 rounded-md bg-background/50 border border-border/50"
                >
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="font-medium text-sm">{holding.symbol}</span>
                      <span className="text-xs text-muted-foreground truncate">
                        {holding.org_name}
                      </span>
                    </div>
                    <div className="text-xs text-muted-foreground">
                      <span className="text-foreground font-medium">
                        {formatBalance(holding.total_unlocked_coin)}
                      </span>
                      {parseFloat(holding.total_locked_coin) > 0 && (
                        <span className="ml-2 text-brand-yellow">
                          ({formatBalance(holding.total_locked_coin)} locked)
                        </span>
                      )}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}

          {/* Ledger API Holdings (Solflare/Phantom) */}
          {!holdingsLoading && !holdingsError && ledgerHoldings.length > 0 && (
            <div className="space-y-2">
              {ledgerHoldings.map((holding, index) => (
                <div
                  key={`${holding.contractId}-${index}`}
                  className="flex items-center gap-3 p-2 rounded-md bg-background/50 border border-border/50"
                >
                  <Coins className="w-6 h-6 text-brand-yellow" />
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="font-medium text-sm">{holding.tokenId}</span>
                      {holding.isLocked && (
                        <span className="text-xs text-brand-yellow">(locked)</span>
                      )}
                    </div>
                    <div className="text-xs text-muted-foreground">
                      <span className="text-foreground font-medium">
                        {formatBalance(holding.amount)}
                      </span>
                    </div>
                    <div className="text-xs text-muted-foreground truncate" title={holding.contractId}>
                      Contract: {holding.contractId.slice(0, 16)}...
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Canton Coin Holdings Section */}
        <div className="space-y-2 p-3 rounded-md bg-muted/30 border border-border backdrop-blur-sm">
          <div className="flex items-center justify-between">
            <h4 className="text-sm font-semibold text-foreground flex items-center">
              <Coins className="w-4 h-4 mr-2" />
              Canton Coin Holdings
            </h4>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={() => fetchActiveContracts()}
              disabled={contractsLoading}
              className="h-6 w-6 p-0"
            >
              <RefreshCw className={`h-3 w-3 ${contractsLoading ? "animate-spin" : ""}`} />
            </Button>
          </div>

          {contractsLoading && (
            <div className="flex flex-col items-center justify-center py-4 gap-2">
              <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
              <p className="text-xs text-muted-foreground">Loading holdings...</p>
            </div>
          )}

          {contractsError && (
            <Alert variant="destructive" className="p-2">
              <XCircle className="h-4 w-4" />
              <AlertDescription className="text-xs">{contractsError}</AlertDescription>
            </Alert>
          )}

          {!contractsLoading && !contractsError && amuletContracts.length === 0 && (
            <p className="text-xs text-muted-foreground text-center py-2">
              No Canton Coin holdings found
            </p>
          )}

          {!contractsLoading && !contractsError && amuletContracts.length > 0 && (
            <div className="space-y-2">
              {amuletContracts.map((contract, index) => (
                <div
                  key={`${contract.contractId}-${index}`}
                  className="p-2 rounded-md bg-background/50 border border-border/50"
                >
                  <div className="space-y-1">
                    {/* Amount prominently displayed with download button */}
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-2">
                        <Coins className="w-4 h-4 text-brand-yellow" />
                        <span className="text-sm font-semibold text-foreground">
                          {formatBalance(contract.amount.initialAmount)} CC
                        </span>
                      </div>
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        onClick={() => downloadContractJson(contract)}
                        className="h-6 w-6 p-0"
                        title="Download contract JSON"
                      >
                        <Download className="h-3 w-3" />
                      </Button>
                    </div>
                    {contract.packageName && (
                      <div className="flex items-center gap-2">
                        <span className="text-xs text-muted-foreground">Package:</span>
                        <span className="text-xs font-medium">{contract.packageName}</span>
                      </div>
                    )}
                    {contract.templateId && (
                      <div className="flex items-center gap-2">
                        <span className="text-xs text-muted-foreground">Template:</span>
                        <span className="text-xs font-mono truncate flex-1" title={contract.templateId}>
                          {contract.templateId.split(":").slice(-2).join(":")}
                        </span>
                      </div>
                    )}
                    {contract.contractId && (
                      <div className="flex items-start gap-2">
                        <span className="text-xs text-muted-foreground shrink-0">Contract:</span>
                        <span
                          className="text-xs font-mono break-all flex-1 cursor-pointer hover:text-primary"
                          title="Click to copy"
                          onClick={() => navigator.clipboard.writeText(contract.contractId)}
                        >
                          {contract.contractId}
                        </span>
                      </div>
                    )}
                    {contract.createdAt && (
                      <div className="flex items-center gap-2">
                        <span className="text-xs text-muted-foreground">Created:</span>
                        <span className="text-xs">
                          {new Date(contract.createdAt).toLocaleString()}
                        </span>
                      </div>
                    )}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* CIP-56 Holdings Section */}
        <div className="space-y-2 p-3 rounded-md bg-muted/30 border border-border backdrop-blur-sm">
          <div className="flex items-center justify-between">
            <h4 className="text-sm font-semibold text-foreground flex items-center">
              <Coins className="w-4 h-4 mr-2" />
              CIP-56 Holdings
            </h4>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={() => fetchCIP56Holdings()}
              disabled={cip56HoldingsLoading}
              className="h-6 w-6 p-0"
            >
              <RefreshCw className={`h-3 w-3 ${cip56HoldingsLoading ? "animate-spin" : ""}`} />
            </Button>
          </div>

          {cip56HoldingsLoading && (
            <div className="flex flex-col items-center justify-center py-4 gap-2">
              <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
              <p className="text-xs text-muted-foreground">Loading CIP-56 holdings...</p>
            </div>
          )}

          {cip56HoldingsError && (
            <Alert variant="destructive" className="p-2">
              <XCircle className="h-4 w-4" />
              <AlertDescription className="text-xs">{cip56HoldingsError}</AlertDescription>
            </Alert>
          )}

          {!cip56HoldingsLoading && !cip56HoldingsError && cip56Holdings.length === 0 && (
            <p className="text-xs text-muted-foreground text-center py-2">
              No CIP-56 compatible holdings found
            </p>
          )}

          {!cip56HoldingsLoading && !cip56HoldingsError && cip56Holdings.length > 0 && (
            <div className="space-y-2">
              {cip56Holdings.map((holding, index) => {
                const tokenMeta = getHoldingMetadata(holding);
                return (
                <div
                  key={`${holding.contractId}-${index}`}
                  className="p-2 rounded-md bg-background/50 border border-border/50"
                >
                  <div className="space-y-1">
                    {/* Amount and instrument prominently displayed */}
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-2">
                        {tokenMeta?.logoUrl ? (
                          <Image
                            src={tokenMeta.logoUrl}
                            alt={tokenMeta.symbol || holding.instrumentId || "Token"}
                            width={16}
                            height={16}
                            className="w-4 h-4 rounded-full"
                            onError={(e) => {
                              // Fall back to Coins icon on image load error
                              e.currentTarget.style.display = "none";
                              e.currentTarget.nextElementSibling?.classList.remove("hidden");
                            }}
                          />
                        ) : null}
                        <Coins className={`w-4 h-4 text-brand-purple ${tokenMeta?.logoUrl ? "hidden" : ""}`} />
                        <span className="text-sm font-semibold text-foreground">
                          {formatBalance(holding.amount)} {tokenMeta?.symbol || holding.instrumentId || holding.label || "Token"}
                        </span>
                      </div>
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        onClick={() => {
                          const data = {
                            templateId: holding.templateId,
                            contractId: holding.contractId,
                            createdEventBlob: holding.createdEventBlob,
                            instrumentId: holding.instrumentId,
                            instrumentAdmin: holding.instrumentAdmin,
                            amount: holding.amount,
                            owner: holding.owner,
                            label: holding.label,
                          };
                          const blob = new Blob([JSON.stringify(data, null, 2)], { type: "application/json" });
                          const url = URL.createObjectURL(blob);
                          const a = document.createElement("a");
                          a.href = url;
                          a.download = `cip56-holding-${holding.contractId.slice(0, 8)}.json`;
                          a.click();
                          URL.revokeObjectURL(url);
                        }}
                        className="h-6 w-6 p-0"
                        title="Download contract JSON"
                      >
                        <Download className="h-3 w-3" />
                      </Button>
                    </div>
                    {holding.label && (
                      <div className="flex items-center gap-2">
                        <span className="text-xs text-muted-foreground">Label:</span>
                        <span className="text-xs font-medium">{holding.label}</span>
                      </div>
                    )}
                    {holding.instrumentAdmin && (
                      <div className="flex items-center gap-2">
                        <span className="text-xs text-muted-foreground">Admin:</span>
                        <span
                          className="text-xs font-mono truncate flex-1 cursor-pointer hover:text-primary"
                          title="Click to copy"
                          onClick={() => navigator.clipboard.writeText(holding.instrumentAdmin)}
                        >
                          {holding.instrumentAdmin}
                        </span>
                      </div>
                    )}
                    {holding.owner && (
                      <div className="flex items-center gap-2">
                        <span className="text-xs text-muted-foreground">Owner:</span>
                        <span
                          className="text-xs font-mono truncate flex-1 cursor-pointer hover:text-primary"
                          title="Click to copy"
                          onClick={() => navigator.clipboard.writeText(holding.owner)}
                        >
                          {holding.owner}
                        </span>
                      </div>
                    )}
                    {holding.isLocked && (
                      <div className="flex items-center gap-2">
                        <span className="text-xs text-muted-foreground">Status:</span>
                        <span className="text-xs text-brand-yellow">Locked</span>
                      </div>
                    )}
                    {holding.templateId && (
                      <div className="flex items-center gap-2">
                        <span className="text-xs text-muted-foreground">Template:</span>
                        <span className="text-xs font-mono truncate flex-1" title={holding.templateId}>
                          {holding.templateId.split(":").slice(-2).join(":")}
                        </span>
                      </div>
                    )}
                    {holding.contractId && (
                      <div className="flex items-start gap-2">
                        <span className="text-xs text-muted-foreground shrink-0">Contract:</span>
                        <span
                          className="text-xs font-mono break-all flex-1 cursor-pointer hover:text-primary"
                          title="Click to copy"
                          onClick={() => navigator.clipboard.writeText(holding.contractId)}
                        >
                          {holding.contractId}
                        </span>
                      </div>
                    )}
                  </div>
                </div>
              );
              })}
            </div>
          )}
        </div>

        {/* Preapproval Contracts Section */}
        <div className="space-y-2 p-3 rounded-md bg-muted/30 border border-border backdrop-blur-sm">
          <div className="flex items-center justify-between">
            <h4 className="text-sm font-semibold text-foreground flex items-center">
              <UserCheck className="w-4 h-4 mr-2" />
              Preapproval Status
            </h4>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={() => fetchPreapprovalContracts()}
              disabled={preapprovalContractsLoading}
              className="h-6 w-6 p-0"
            >
              <RefreshCw className={`h-3 w-3 ${preapprovalContractsLoading ? "animate-spin" : ""}`} />
            </Button>
          </div>

          {preapprovalContractsLoading && (
            <div className="flex flex-col items-center justify-center py-4 gap-2">
              <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
              <p className="text-xs text-muted-foreground">Loading preapproval status...</p>
            </div>
          )}

          {!preapprovalContractsLoading && acceptedPreapprovals.length === 0 && (
            <p className="text-xs text-muted-foreground text-center py-2">
              No active preapproval found. Create a proposal below.
            </p>
          )}

          {/* Active Preapprovals */}
          {!preapprovalContractsLoading && acceptedPreapprovals.length > 0 && (
            <div className="space-y-2">
              {acceptedPreapprovals.map((preapproval, index) => (
                <div key={preapproval.contractId || index} className="p-2 rounded-md bg-brand-green/10 border border-brand-green/30">
                  <div className="space-y-1">
                    <div className="flex items-center gap-2">
                      <span className="text-xs text-muted-foreground">Status:</span>
                      <span className="text-xs font-medium text-brand-green">Active</span>
                    </div>
                    {preapproval.provider && (
                      <div className="flex items-start gap-2">
                        <span className="text-xs text-muted-foreground shrink-0">Provider:</span>
                        <span
                          className="text-xs font-mono break-all flex-1 cursor-pointer hover:text-primary"
                          title="Click to copy"
                          onClick={() => navigator.clipboard.writeText(preapproval.provider)}
                        >
                          {preapproval.provider}
                        </span>
                      </div>
                    )}
                    {preapproval.expiresAt && (
                      <div className="flex items-center gap-2">
                        <span className="text-xs text-muted-foreground">Expires:</span>
                        <span className="text-xs">
                          {new Date(preapproval.expiresAt).toLocaleDateString()}
                        </span>
                      </div>
                    )}
                    {preapproval.contractId && (
                      <div className="flex items-start gap-2">
                        <span className="text-xs text-muted-foreground shrink-0">Contract:</span>
                        <span
                          className="text-xs font-mono break-all flex-1 cursor-pointer hover:text-primary"
                          title="Click to copy"
                          onClick={() => navigator.clipboard.writeText(preapproval.contractId)}
                        >
                          {preapproval.contractId}
                        </span>
                      </div>
                    )}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* User Service Status Section */}
        <div className="space-y-2 p-3 rounded-md bg-muted/30 border border-border backdrop-blur-sm">
          <div className="flex items-center justify-between">
            <h4 className="text-sm font-semibold text-foreground flex items-center">
              <UserCheck className="w-4 h-4 mr-2" />
              User Service Status
            </h4>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={() => fetchUserServiceStatus()}
              disabled={userServiceStatusLoading}
              className="h-6 w-6 p-0"
            >
              <RefreshCw className={`h-3 w-3 ${userServiceStatusLoading ? "animate-spin" : ""}`} />
            </Button>
          </div>

          {userServiceStatusLoading && (
            <div className="flex flex-col items-center justify-center py-4 gap-2">
              <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
              <p className="text-xs text-muted-foreground">Loading user service status...</p>
            </div>
          )}

          {!userServiceStatusLoading && userServices.length === 0 && userServiceRequests.length === 0 && (
            <p className="text-xs text-muted-foreground text-center py-2">
              No active user services or pending requests found.
            </p>
          )}

          {/* Active UserService contracts */}
          {!userServiceStatusLoading && userServices.length > 0 && (
            <div className="space-y-2">
              <p className="text-xs font-medium text-brand-green">Active Services ({userServices.length})</p>
              {userServices.map((service, index) => (
                <div key={service.contractId || index} className="p-2 rounded-md bg-brand-green/10 border border-brand-green/30">
                  <div className="space-y-1">
                    <div className="flex items-center gap-2">
                      <span className="text-xs text-muted-foreground">Status:</span>
                      <span className="text-xs font-medium text-brand-green">Active</span>
                    </div>
                    {service.operator && (
                      <div className="flex items-start gap-2">
                        <span className="text-xs text-muted-foreground shrink-0">Operator:</span>
                        <span
                          className="text-xs font-mono break-all flex-1 cursor-pointer hover:text-primary"
                          title="Click to copy"
                          onClick={() => navigator.clipboard.writeText(service.operator)}
                        >
                          {service.operator}
                        </span>
                      </div>
                    )}
                    {service.contractId && (
                      <div className="flex items-start gap-2">
                        <span className="text-xs text-muted-foreground shrink-0">Contract:</span>
                        <span
                          className="text-xs font-mono break-all flex-1 cursor-pointer hover:text-primary"
                          title="Click to copy"
                          onClick={() => navigator.clipboard.writeText(service.contractId)}
                        >
                          {service.contractId}
                        </span>
                      </div>
                    )}
                  </div>
                </div>
              ))}
            </div>
          )}

          {/* Pending UserServiceRequest contracts */}
          {!userServiceStatusLoading && userServiceRequests.length > 0 && (
            <div className="space-y-2">
              <p className="text-xs font-medium text-brand-yellow">Pending Requests ({userServiceRequests.length})</p>
              {userServiceRequests.map((request, index) => (
                <div key={request.contractId || index} className="p-2 rounded-md bg-brand-yellow/10 border border-brand-yellow/30">
                  <div className="space-y-1">
                    <div className="flex items-center gap-2">
                      <span className="text-xs text-muted-foreground">Status:</span>
                      <span className="text-xs font-medium text-brand-yellow">Pending</span>
                    </div>
                    {request.operator && (
                      <div className="flex items-start gap-2">
                        <span className="text-xs text-muted-foreground shrink-0">Operator:</span>
                        <span
                          className="text-xs font-mono break-all flex-1 cursor-pointer hover:text-primary"
                          title="Click to copy"
                          onClick={() => navigator.clipboard.writeText(request.operator)}
                        >
                          {request.operator}
                        </span>
                      </div>
                    )}
                    {request.contractId && (
                      <div className="flex items-start gap-2">
                        <span className="text-xs text-muted-foreground shrink-0">Contract:</span>
                        <span
                          className="text-xs font-mono break-all flex-1 cursor-pointer hover:text-primary"
                          title="Click to copy"
                          onClick={() => navigator.clipboard.writeText(request.contractId)}
                        >
                          {request.contractId}
                        </span>
                      </div>
                    )}
                  </div>
                </div>
              ))}
            </div>
          )}

          {/* Credential Offers */}
          {!userServiceStatusLoading && credentialOffers.length > 0 && (
            <div className="space-y-2">
              <p className="text-xs font-medium text-brand-blue">Credential Offers ({credentialOffers.length})</p>
              {credentialOffers.map((offer, index) => {
                const isPaid = !!offer.billingParams || !!offer.depositInitialAmountUsd;
                return (
                  <div key={offer.contractId || index} className="p-2 rounded-md bg-brand-blue/10 border border-brand-blue/30">
                    <div className="space-y-1">
                      <div className="flex items-center justify-between">
                        <div className="flex items-center gap-2">
                          <span className="text-xs text-muted-foreground">Type:</span>
                          <span className={`text-xs font-medium ${isPaid ? "text-brand-yellow" : "text-brand-green"}`}>
                            {isPaid ? "Paid" : "Free"}
                          </span>
                        </div>
                        <Button
                          type="button"
                          variant="outline"
                          size="sm"
                          onClick={() => handleAcceptCredentialOffer(offer)}
                          disabled={acceptingOfferId === offer.contractId || walletType !== "loop"}
                          className="h-6 text-xs px-2"
                        >
                          {acceptingOfferId === offer.contractId ? (
                            <Loader2 className="h-3 w-3 animate-spin" />
                          ) : (
                            <>
                              <CheckCircle className="h-3 w-3 mr-1" />
                              Accept
                            </>
                          )}
                        </Button>
                      </div>
                      {offer.description && (
                        <div className="flex items-start gap-2">
                          <span className="text-xs text-muted-foreground shrink-0">Description:</span>
                          <span className="text-xs flex-1">{offer.description}</span>
                        </div>
                      )}
                      {offer.issuer && (
                        <div className="flex items-start gap-2">
                          <span className="text-xs text-muted-foreground shrink-0">Issuer:</span>
                          <span
                            className="text-xs font-mono break-all flex-1 cursor-pointer hover:text-primary"
                            title="Click to copy"
                            onClick={() => navigator.clipboard.writeText(offer.issuer)}
                          >
                            {offer.issuer}
                          </span>
                        </div>
                      )}
                      {isPaid && offer.billingParams?.feePerDayUsd?.rate && (
                        <div className="flex items-center gap-2">
                          <span className="text-xs text-muted-foreground">Monthly Fee:</span>
                          <span className="text-xs font-medium">
                            ${(parseFloat(offer.billingParams.feePerDayUsd.rate) * 365 / 12).toFixed(2)} USD
                          </span>
                        </div>
                      )}
                      {isPaid && offer.depositInitialAmountUsd && (
                        <div className="flex items-center gap-2">
                          <span className="text-xs text-muted-foreground">Deposit:</span>
                          <span className="text-xs font-medium">${parseFloat(offer.depositInitialAmountUsd).toFixed(2)} USD</span>
                        </div>
                      )}
                      {offer.contractId && (
                        <div className="flex items-start gap-2">
                          <span className="text-xs text-muted-foreground shrink-0">Contract:</span>
                          <span
                            className="text-xs font-mono break-all flex-1 cursor-pointer hover:text-primary"
                            title="Click to copy"
                            onClick={() => navigator.clipboard.writeText(offer.contractId)}
                          >
                            {offer.contractId}
                          </span>
                        </div>
                      )}
                    </div>
                  </div>
                );
              })}
              {acceptOfferResult && !acceptOfferResult.success && (
                <Alert variant="destructive" className="mt-2 p-2">
                  <XCircle className="h-4 w-4" />
                  <AlertTitle className="text-sm">Accept Failed</AlertTitle>
                  <AlertDescription className="text-xs">
                    {acceptOfferResult.error || "Unknown error occurred"}
                  </AlertDescription>
                </Alert>
              )}
              {acceptOfferResult?.success && (
                <Alert variant="default" className="mt-2 p-2 bg-muted/30 border-border">
                  <CheckCircle className="h-4 w-4 text-brand-green" />
                  <AlertTitle className="text-sm text-foreground">Offer Accepted</AlertTitle>
                  <AlertDescription className="text-xs text-muted-foreground">
                    Credential offer accepted successfully.
                  </AlertDescription>
                </Alert>
              )}
            </div>
          )}

          {/* Active Credentials (Subscriptions) */}
          {!userServiceStatusLoading && credentials.length > 0 && (
            <div className="space-y-2">
              <p className="text-xs font-medium text-brand-green">Active Credentials ({credentials.length})</p>
              {credentials.map((cred, index) => {
                // Find matching billing contract for this credential
                const billing = credentialBillings.find(b => b.credentialId === cred.id);
                const isPaid = !!billing;

                return (
                  <div key={cred.contractId || index} className="p-2 rounded-md bg-brand-green/10 border border-brand-green/30">
                    <div className="space-y-1">
                      <div className="flex items-center justify-between">
                        <div className="flex items-center gap-2">
                          <ShieldCheck className="w-4 h-4 text-brand-green" />
                          <span className="text-xs font-medium text-foreground">{cred.id}</span>
                        </div>
                        <span className={`text-xs font-medium ${isPaid ? "text-brand-yellow" : "text-brand-green"}`}>
                          {isPaid ? "Paid" : "Free"}
                        </span>
                      </div>
                      {cred.description && (
                        <div className="flex items-start gap-2">
                          <span className="text-xs text-muted-foreground shrink-0">Description:</span>
                          <span className="text-xs flex-1">{cred.description}</span>
                        </div>
                      )}
                      {cred.issuer && (
                        <div className="flex items-start gap-2">
                          <span className="text-xs text-muted-foreground shrink-0">Issuer:</span>
                          <span
                            className="text-xs font-mono break-all flex-1 cursor-pointer hover:text-primary"
                            title="Click to copy"
                            onClick={() => navigator.clipboard.writeText(cred.issuer)}
                          >
                            {cred.issuer}
                          </span>
                        </div>
                      )}
                      {/* Billing info for paid credentials */}
                      {billing && (
                        <>
                          <div className="flex items-center gap-2">
                            <span className="text-xs text-muted-foreground">Status:</span>
                            <span className={`text-xs font-medium ${
                              billing.billingState.status === "Success" ? "text-brand-green" :
                              billing.billingState.status === "Failure" ? "text-destructive" :
                              "text-brand-yellow"
                            }`}>
                              {billing.billingState.status}
                            </span>
                          </div>
                          <div className="flex items-center gap-2">
                            <span className="text-xs text-muted-foreground">Current Deposit:</span>
                            <span className="text-xs font-medium">
                              {parseFloat(billing.balanceState.currentDepositAmountCc).toFixed(4)} CC
                            </span>
                          </div>
                          {billing.billingState.billedUntil && (
                            <div className="flex items-center gap-2">
                              <span className="text-xs text-muted-foreground">Billed Until:</span>
                              <span className="text-xs">
                                {new Date(billing.billingState.billedUntil).toLocaleDateString()}
                              </span>
                            </div>
                          )}
                          <div className="flex items-center gap-2">
                            <span className="text-xs text-muted-foreground">Monthly Fee:</span>
                            <span className="text-xs font-medium">
                              ${(parseFloat(billing.params.feePerDayUsd.rate) * 365 / 12).toFixed(2)} USD
                            </span>
                          </div>
                        </>
                      )}
                      {/* Claims */}
                      {cred.claims && cred.claims.length > 0 && (
                        <div className="mt-1 p-1.5 rounded bg-background/50">
                          <span className="text-xs text-muted-foreground">Claims:</span>
                          <div className="mt-1 space-y-0.5">
                            {cred.claims.map((claim, i) => (
                              <div key={i} className="text-xs">
                                <span className="text-muted-foreground">{claim.property}:</span>{" "}
                                <span className="font-medium">{claim.value}</span>
                              </div>
                            ))}
                          </div>
                        </div>
                      )}
                      {cred.contractId && (
                        <div className="flex items-start gap-2">
                          <span className="text-xs text-muted-foreground shrink-0">Contract:</span>
                          <span
                            className="text-xs font-mono break-all flex-1 cursor-pointer hover:text-primary"
                            title="Click to copy"
                            onClick={() => navigator.clipboard.writeText(cred.contractId)}
                          >
                            {cred.contractId}
                          </span>
                        </div>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* Transfer CC Section */}
        <div className="space-y-2 p-3 rounded-md bg-muted/30 border border-border backdrop-blur-sm">
          <h4 className="text-sm font-semibold text-foreground flex items-center">
            <Send className="w-4 h-4 mr-2" />
            Transfer CC
          </h4>
          <div className="space-y-2">
            <div>
              <label className="text-xs text-muted-foreground">Receiver Party ID</label>
              <Input
                placeholder="Enter receiver's party ID..."
                value={transferReceiver}
                onChange={(e) => setTransferReceiver(e.target.value)}
                className="bg-input border-border focus:border-primary text-foreground text-xs placeholder:text-muted-foreground h-8 mt-1"
              />
            </div>
            <div>
              <label className="text-xs text-muted-foreground">Amount (CC)</label>
              <Input
                type="number"
                step="0.01"
                min="0"
                placeholder="0.00"
                value={transferAmount}
                onChange={(e) => setTransferAmount(e.target.value)}
                className="bg-input border-border focus:border-primary text-foreground text-xs placeholder:text-muted-foreground h-8 mt-1"
              />
            </div>
            <div>
              <label className="text-xs text-muted-foreground">Description (optional)</label>
              <Input
                placeholder="Transfer reason..."
                value={transferDescription}
                onChange={(e) => setTransferDescription(e.target.value)}
                className="bg-input border-border focus:border-primary text-foreground text-xs placeholder:text-muted-foreground h-8 mt-1"
              />
            </div>
          </div>
          <Button
            onClick={handleTransfer}
            disabled={!transferReceiver || !transferAmount || transferStatus === "loading" || transferStatus === "awaiting"}
            className="w-full bg-gradient-to-r from-brand-pink via-brand-purple to-brand-blue hover:brightness-105 text-white h-8 text-xs"
          >
            {transferStatus === "loading" && (
              <Loader2 className="mr-2 h-3 w-3 animate-spin" />
            )}
            <Send className="mr-2 h-3 w-3" />
            Send CC
          </Button>
          {transferStatus === "awaiting" && (
            <Alert className="mt-2 p-2 bg-brand-yellow/10 border-brand-yellow/30">
              <Loader2 className="h-4 w-4 animate-spin text-brand-yellow" />
              <AlertTitle className="text-sm text-foreground">Awaiting Approval</AlertTitle>
              <AlertDescription className="text-xs">
                <p className="text-muted-foreground mb-2">
                  Please approve this transaction in your {walletName} Wallet.
                </p>
                <Button
                  variant="link"
                  size="sm"
                  onClick={openLoopWallet}
                  className="h-6 p-0 text-xs text-brand-yellow hover:text-brand-yellow/80"
                >
                  Open {walletName} Wallet
                  <ExternalLink className="ml-1 h-3 w-3" />
                </Button>
              </AlertDescription>
            </Alert>
          )}
          {transferStatus === "error" && transferResult && (
            <Alert variant="destructive" className="mt-2 p-2">
              <XCircle className="h-4 w-4" />
              <AlertTitle className="text-sm">Transfer Failed</AlertTitle>
              <AlertDescription className="text-xs">
                {transferResult.error || "Unknown error occurred"}
              </AlertDescription>
            </Alert>
          )}
          {transferStatus === "success" && transferResult && (
            <Alert
              variant="default"
              className="mt-2 p-2 bg-muted/30 border-border"
            >
              <CheckCircle className="h-4 w-4 text-brand-green" />
              <AlertTitle className="text-sm text-foreground">
                Transfer Successful
              </AlertTitle>
              <AlertDescription className="space-y-1 text-xs">
                <p className="text-muted-foreground">
                  Your CC has been sent successfully.
                </p>
              </AlertDescription>
            </Alert>
          )}
        </div>

        {/* Preapprove Transfers Section */}
        <div className="space-y-2 p-3 rounded-md bg-muted/30 border border-border backdrop-blur-sm">
          <h4 className="text-sm font-semibold text-foreground flex items-center">
            <UserCheck className="w-4 h-4 mr-2" />
            Preapprove Transfers
            {walletType && (
              <span className="ml-2 text-xs font-normal text-muted-foreground">
                ({walletType === "phantom" ? "Phantom signing" : walletType === "solflare" ? "Solflare signing" : "Loop signing"})
              </span>
            )}
          </h4>
          <p className="text-xs text-muted-foreground">
            Create a transfer preapproval proposal{walletType === "phantom" || walletType === "solflare" ? ` using ${walletType === "phantom" ? "Phantom" : "Solflare"} wallet signature` : ""}.
          </p>
          <div>
            <label className="text-xs text-muted-foreground">Provider Party ID</label>
            <Input
              placeholder="Enter provider's party ID..."
              value={preapprovalProvider}
              onChange={(e) => setPreapprovalProvider(e.target.value)}
              className="bg-input border-border focus:border-primary text-foreground text-xs placeholder:text-muted-foreground h-8 mt-1"
            />
          </div>
          <Button
            onClick={handleCreatePreapproval}
            disabled={!preapprovalProvider.trim() || preapprovalStatus === "loading" || preapprovalStatus === "awaiting"}
            className="w-full bg-gradient-to-r from-brand-pink via-brand-purple to-brand-blue hover:brightness-105 text-white h-8 text-xs"
          >
            {preapprovalStatus === "loading" && (
              <Loader2 className="mr-2 h-3 w-3 animate-spin" />
            )}
            <UserCheck className="mr-2 h-3 w-3" />
            Create Preapproval Proposal
          </Button>
          {preapprovalStatus === "awaiting" && (
            <Alert className="mt-2 p-2 bg-brand-yellow/10 border-brand-yellow/30">
              <Loader2 className="h-4 w-4 animate-spin text-brand-yellow" />
              <AlertTitle className="text-sm text-foreground">Awaiting Approval</AlertTitle>
              <AlertDescription className="text-xs">
                {walletType === "phantom" || walletType === "solflare" ? (
                  <p className="text-muted-foreground">
                    Please sign the transaction in the {walletType === "phantom" ? "Phantom" : "Solflare"} popup.
                  </p>
                ) : (
                  <>
                    <p className="text-muted-foreground mb-2">
                      Please approve this transaction in your {walletName} Wallet.
                    </p>
                    <Button
                      variant="link"
                      size="sm"
                      onClick={openLoopWallet}
                      className="h-6 p-0 text-xs text-brand-yellow hover:text-brand-yellow/80"
                    >
                      Open {walletName} Wallet
                      <ExternalLink className="ml-1 h-3 w-3" />
                    </Button>
                  </>
                )}
              </AlertDescription>
            </Alert>
          )}
          {preapprovalStatus === "error" && preapprovalResult && (
            <Alert variant="destructive" className="mt-2 p-2">
              <XCircle className="h-4 w-4" />
              <AlertTitle className="text-sm">Preapproval Failed</AlertTitle>
              <AlertDescription className="text-xs">
                {preapprovalResult.error || "Unknown error occurred"}
              </AlertDescription>
            </Alert>
          )}
          {preapprovalStatus === "success" && preapprovalResult && (
            <Alert
              variant="default"
              className="mt-2 p-2 bg-muted/30 border-border"
            >
              <CheckCircle className="h-4 w-4 text-brand-green" />
              <AlertTitle className="text-sm text-foreground">
                Proposal Created
              </AlertTitle>
              <AlertDescription className="space-y-1 text-xs">
                <p className="text-muted-foreground">
                  Transfer preapproval proposal has been created.
                  The provider needs to accept it to complete the preapproval.
                </p>
              </AlertDescription>
            </Alert>
          )}
        </div>

        {/* Accept Advanced Payment Request Section (Solflare and Loop) */}
        {(walletType === "solflare" || walletType === "loop") && (
          <div className="space-y-2 p-3 rounded-md bg-muted/30 border border-border backdrop-blur-sm">
            <h4 className="text-sm font-semibold text-foreground flex items-center">
              <CheckCircle className="w-4 h-4 mr-2" />
              Accept Advanced Payment Request
              {walletType && (
                <span className="ml-2 text-xs font-normal text-muted-foreground">
                  ({walletType === "loop" ? "Loop signing" : "Solflare signing"})
                </span>
              )}
            </h4>
            <p className="text-xs text-muted-foreground">
              Accept an AdvancedPaymentRequest by locking your CC. Your unlocked Amulet contracts will be automatically used.
            </p>
            <div>
              <label className="text-xs text-muted-foreground">Request Contract ID</label>
              <Input
                placeholder="Enter request contract ID..."
                value={acceptRequestCid}
                onChange={(e) => setAcceptRequestCid(e.target.value)}
                className="bg-input border-border focus:border-primary text-foreground font-mono text-xs placeholder:text-muted-foreground h-8 mt-1"
              />
            </div>
            <Button
              onClick={handleAcceptRequest}
              disabled={!acceptRequestCid.trim() || acceptRequestStatus === "loading" || acceptRequestStatus === "awaiting"}
              className="w-full bg-gradient-to-r from-brand-green via-brand-blue to-brand-purple hover:brightness-105 text-white h-8 text-xs"
            >
              {acceptRequestStatus === "loading" && (
                <Loader2 className="mr-2 h-3 w-3 animate-spin" />
              )}
              <CheckCircle className="mr-2 h-3 w-3" />
              Accept Request
            </Button>
            {acceptRequestStatus === "awaiting" && (
              <Alert className="mt-2 p-2 bg-brand-yellow/10 border-brand-yellow/30">
                <Loader2 className="h-4 w-4 animate-spin text-brand-yellow" />
                <AlertTitle className="text-sm text-foreground">Awaiting Signature</AlertTitle>
                <AlertDescription className="text-xs">
                  <p className="text-muted-foreground">
                    {walletType === "loop"
                      ? "Please approve the signature in your Loop Wallet."
                      : "Please sign the transaction in the Solflare popup."}
                  </p>
                </AlertDescription>
              </Alert>
            )}
            {acceptRequestStatus === "error" && acceptRequestResult && (
              <Alert variant="destructive" className="mt-2 p-2">
                <XCircle className="h-4 w-4" />
                <AlertTitle className="text-sm">Accept Failed</AlertTitle>
                <AlertDescription className="text-xs">
                  {acceptRequestResult.error || "Unknown error occurred"}
                </AlertDescription>
              </Alert>
            )}
            {acceptRequestStatus === "success" && acceptRequestResult && (
              <Alert
                variant="default"
                className="mt-2 p-2 bg-muted/30 border-border"
              >
                <CheckCircle className="h-4 w-4 text-brand-green" />
                <AlertTitle className="text-sm text-foreground">
                  Request Accepted
                </AlertTitle>
                <AlertDescription className="space-y-1 text-xs">
                  <p className="text-muted-foreground">
                    AdvancedPayment contract created. Your CC has been locked.
                  </p>
                  {acceptRequestResult.submissionId && (
                    <div className="flex items-start gap-2">
                      <span className="text-muted-foreground shrink-0">Submission:</span>
                      <span
                        className="font-mono break-all flex-1 cursor-pointer hover:text-primary"
                        title="Click to copy"
                        onClick={() => navigator.clipboard.writeText(acceptRequestResult.submissionId!)}
                      >
                        {acceptRequestResult.submissionId}
                      </span>
                    </div>
                  )}
                  {acceptRequestResult.updateId && (
                    <div className="flex items-start gap-2">
                      <span className="text-muted-foreground shrink-0">Update ID:</span>
                      <span
                        className="font-mono break-all flex-1 cursor-pointer hover:text-primary"
                        title="Click to copy"
                        onClick={() => navigator.clipboard.writeText(acceptRequestResult.updateId!)}
                      >
                        {acceptRequestResult.updateId}
                      </span>
                    </div>
                  )}
                </AlertDescription>
              </Alert>
            )}
          </div>
        )}

        {/* Sign Message Section */}
        <div className="space-y-2 p-3 rounded-md bg-muted/30 border border-border backdrop-blur-sm">
          <h4 className="text-sm font-semibold text-foreground flex items-center">
            <Edit3 className="w-4 h-4 mr-2" />
            Sign Message
          </h4>
          <Textarea
            placeholder="Enter message to sign..."
            value={messageToSign}
            onChange={(e) => setMessageToSign(e.target.value)}
            className="bg-input border-border focus:border-primary text-foreground text-xs placeholder:text-muted-foreground"
            rows={2}
          />
          <Button
            onClick={handleSignMessage}
            disabled={!messageToSign || signStatus === "loading"}
            className="w-full bg-gradient-to-r from-brand-pink via-brand-purple to-brand-blue hover:brightness-105 text-white h-8 text-xs"
          >
            {signStatus === "loading" && (
              <Loader2 className="mr-2 h-3 w-3 animate-spin" />
            )}
            Sign Message
          </Button>
          {signStatus === "error" && (
            <Alert variant="destructive" className="mt-2 p-2">
              <XCircle className="h-4 w-4" />
              <AlertTitle className="text-sm">Error</AlertTitle>
              <AlertDescription className="text-xs">
                Signing failed.
              </AlertDescription>
            </Alert>
          )}
          {signedMessage && signStatus === "success" && (
            <Alert
              variant="default"
              className="mt-2 p-2 bg-muted/30 border-border"
            >
              <CheckCircle className="h-4 w-4 text-brand-green" />
              <AlertTitle className="text-sm text-foreground">
                Message Signed
              </AlertTitle>
              <AlertDescription className="space-y-2 text-xs">
                <DataRow
                  label="Original Message"
                  value={signedMessage.message}
                  truncate={true}
                  className="border-none py-0.5"
                  valueClassName="text-xs"
                />
                <DataRow
                  label="Signature"
                  value={signedMessage.signature}
                  truncate={true}
                  className="border-none py-0.5"
                  valueClassName="text-xs"
                />
                <div className="flex items-center gap-2 pt-1">
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={handleVerifySignature}
                    disabled={verifyStatus === "loading"}
                    className="h-7 text-xs"
                  >
                    {verifyStatus === "loading" ? (
                      <Loader2 className="mr-1 h-3 w-3 animate-spin" />
                    ) : (
                      <ShieldCheck className="mr-1 h-3 w-3" />
                    )}
                    Verify
                  </Button>
                  {verifyStatus === "valid" && (
                    <span className="flex items-center text-brand-green text-xs">
                      <CheckCircle className="h-3 w-3 mr-1" />
                      Valid
                    </span>
                  )}
                  {verifyStatus === "invalid" && (
                    <span className="flex items-center text-destructive text-xs">
                      <XCircle className="h-3 w-3 mr-1" />
                      Invalid
                    </span>
                  )}
                </div>
              </AlertDescription>
            </Alert>
          )}
        </div>

        {/* Sign Multihash Section (Solflare and Loop) */}
        {(walletType === "solflare" || walletType === "loop") && (
          <div className="space-y-2 p-3 rounded-md bg-muted/30 border border-border backdrop-blur-sm">
            <h4 className="text-sm font-semibold text-foreground flex items-center">
              <Edit3 className="w-4 h-4 mr-2" />
              Sign Multihash
              {walletType && (
                <span className="ml-2 text-xs font-normal text-muted-foreground">
                  ({walletType === "loop" ? "Loop signing" : "Solflare signing"})
                </span>
              )}
            </h4>
            <p className="text-xs text-muted-foreground">
              Sign a base64-encoded multihash for topology transactions
            </p>
            <Input
              placeholder="e.g., EiCCb4H1HHMWsFhlXVI9LKbvX/nOZ2tbDgLxLymO6jxZiQ=="
              value={multihashToSign}
              onChange={(e) => setMultihashToSign(e.target.value)}
              className="bg-input border-border focus:border-primary text-foreground font-mono text-xs placeholder:text-muted-foreground h-8"
            />
            <Button
              onClick={handleSignMultihash}
              disabled={!multihashToSign || signMultihashStatus === "loading"}
              className="w-full bg-gradient-to-r from-brand-pink via-brand-purple to-brand-blue hover:brightness-105 text-white h-8 text-xs"
            >
              {signMultihashStatus === "loading" && (
                <Loader2 className="mr-2 h-3 w-3 animate-spin" />
              )}
              Sign Multihash
            </Button>
            {signMultihashStatus === "error" && (
              <Alert variant="destructive" className="mt-2 p-2">
                <XCircle className="h-4 w-4" />
                <AlertTitle className="text-sm">Signing Failed</AlertTitle>
                <AlertDescription className="text-xs">
                  User rejected or signing failed
                </AlertDescription>
              </Alert>
            )}
            {signedMultihash && signMultihashStatus === "success" && (
              <Alert
                variant="default"
                className="mt-2 p-2 bg-muted/30 border-border"
              >
                <CheckCircle className="h-4 w-4 text-brand-green" />
                <AlertTitle className="text-sm text-foreground">
                  Multihash Signed
                </AlertTitle>
                <AlertDescription className="space-y-2 text-xs">
                  <DataRow
                    label="Multihash"
                    value={signedMultihash.multihash}
                    truncate={true}
                    className="border-none py-0.5"
                    valueClassName="text-xs"
                  />
                  <DataRow
                    label="Signature"
                    value={signedMultihash.signature}
                    truncate={true}
                    className="border-none py-0.5"
                    valueClassName="text-xs"
                  />
                </AlertDescription>
              </Alert>
            )}
          </div>
        )}

        {/* Request Service Section (Solflare and Loop) */}
        {(walletType === "solflare" || walletType === "loop") && (
          <div className="space-y-2 p-3 rounded-md bg-muted/30 border border-border backdrop-blur-sm">
            <h4 className="text-sm font-semibold text-foreground flex items-center">
              <UserCheck className="w-4 h-4 mr-2" />
              Request Service
              <span className="ml-2 text-xs font-normal text-muted-foreground">
                ({walletType === "loop" ? "Loop signing" : "Solflare signing"})
              </span>
            </h4>
            <p className="text-xs text-muted-foreground">
              Request a Utility Credential service from an operator
            </p>
            <div>
              <label className="text-xs text-muted-foreground">Operator Party ID</label>
              <Input
                placeholder="Enter operator's party ID..."
                value={requestServiceOperator}
                onChange={(e) => setRequestServiceOperator(e.target.value)}
                className="bg-input border-border focus:border-primary text-foreground font-mono text-xs placeholder:text-muted-foreground h-8 mt-1"
              />
            </div>
            <Button
              onClick={handleRequestService}
              disabled={!requestServiceOperator.trim() || requestServiceStatus === "loading"}
              className="w-full bg-gradient-to-r from-brand-green via-brand-blue to-brand-purple hover:brightness-105 text-white h-8 text-xs"
            >
              {requestServiceStatus === "loading" && (
                <Loader2 className="mr-2 h-3 w-3 animate-spin" />
              )}
              <UserCheck className="mr-2 h-3 w-3" />
              Request Service
            </Button>
            {requestServiceStatus === "error" && requestServiceResult && (
              <Alert variant="destructive" className="mt-2 p-2">
                <XCircle className="h-4 w-4" />
                <AlertTitle className="text-sm">Request Failed</AlertTitle>
                <AlertDescription className="text-xs">
                  {requestServiceResult.error || "Unknown error occurred"}
                </AlertDescription>
              </Alert>
            )}
            {requestServiceStatus === "success" && requestServiceResult && (
              <Alert variant="default" className="mt-2 p-2 bg-muted/30 border-border">
                <CheckCircle className="h-4 w-4 text-brand-green" />
                <AlertTitle className="text-sm text-foreground">
                  Request Created
                </AlertTitle>
                <AlertDescription className="space-y-1 text-xs">
                  <p className="text-muted-foreground">
                    UserServiceRequest created successfully. The operator needs to accept it.
                  </p>
                  {requestServiceResult.updateId && (
                    <div className="flex items-start gap-2">
                      <span className="text-muted-foreground shrink-0">Update ID:</span>
                      <span
                        className="font-mono break-all flex-1 cursor-pointer hover:text-primary"
                        title="Click to copy"
                        onClick={() => navigator.clipboard.writeText(requestServiceResult.updateId!)}
                      >
                        {requestServiceResult.updateId}
                      </span>
                    </div>
                  )}
                </AlertDescription>
              </Alert>
            )}
          </div>
        )}

      </div>
    </StatusCard>
  );
}
